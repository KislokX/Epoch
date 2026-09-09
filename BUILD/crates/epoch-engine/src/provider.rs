//! Providers — who does the thinking (ADR-0007).
//!
//! A Provider is **infrastructure**. The Character is the product (`CHARACTER_BIBLE.md`): Mage
//! is Mage whether a local 14B or a frontier model is behind her, and nothing above this module
//! may ask which. That is why [`Provider`] is a trait with no provider's vocabulary in it — no
//! `model` string format, no API shape, no vendor name.
//!
//! ## Everything here is honest about being offline
//!
//! A Provider that is not reachable is the ordinary state, not an error: Ollama may not be
//! running, a key may not be set, a machine may be offline. [`Provider::probe`] therefore
//! always succeeds and reports what it found. Nothing in Epoch is allowed to claim ONLINE
//! without having asked.
//!
//! ## Why blocking HTTP
//!
//! The Engine is synchronous and this keeps it that way. A turn is one request on one thread,
//! and Ollama's streaming is newline-delimited JSON — reading lines, not a reactor. An async
//! client would pull a runtime into the leanest layer that has managed without one. Contained
//! behind this trait, so trading it later costs one file (Earn Complexity).

use std::collections::BTreeMap;
use std::time::Duration;

use epoch_kernel::{
    Arguments, CapabilityId, Control, ControlKind, Conversation, Descriptor, Parameters, Reasoning,
    Role, Secret, TuningValue, Value, ValueKind,
};
use serde::Serialize;

/// How long to wait for a provider to admit it exists.
///
/// Short on purpose: this runs when a surface asks "who is online?", and a user staring at a
/// spinner because nothing is listening is worse than being told nothing is listening.
const PROBE_TIMEOUT: Duration = Duration::from_millis(1200);

/// How long to wait for the TCP connection itself.
///
/// Set separately, and this is not redundant — it is the whole reason probing is fast.
///
/// A refused connection returns immediately. A **dropped** one does not: a firewall that
/// silently discards packets instead of rejecting them leaves `connect` waiting on the
/// operating system's retry schedule, which on Windows is roughly twenty seconds. Measured on
/// this machine: 22.5s to answer "is Ollama running?", with a whole-call timeout of 1.2s set
/// and doing nothing.
///
/// The whole-call timeout cannot help, because the clock it bounds has not started yet — there
/// is no call until there is a socket. Only `timeout_connect` bounds the socket.
///
/// This is not a test-suite annoyance. It is the Launcher freezing on open, on any machine
/// whose firewall drops rather than rejects.
const CONNECT_TIMEOUT: Duration = Duration::from_millis(700);

/// How long a single turn may take before we give up.
///
/// Generous: a 20B model on a laptop thinks slowly, and cutting it off mid-thought would look
/// like a bug in Epoch rather than the physics of the machine.
const TURN_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// The address could not be used, and `because` says what was observed.
    ///
    /// **`endpoint` is an address and nothing else.** `openai.rs` has said why since it was
    /// written — *the address is what a person can act on: a typo, a server not started, a
    /// machine asleep; the transport's own words say none of those* — and the Bridge was the one
    /// place that did not follow it, pasting `ureq`'s raw text in after the address. Measured in
    /// the window on 2026-09-07, stopping EpochServices mid-answer put this in front of a
    /// person:
    ///
    /// ```text
    /// … is not reachable at https://10.0.1.20:11500 (https://10.0.1.20:11500/ask: Network
    /// Error: Network Error: Error encountered in the status line: peer closed connection
    /// without sending TLS close_notify:
    /// https://docs.rs/rustls/latest/rustls/manual/_03_howto/index.html#unexpected-eof)
    /// ```
    ///
    /// The first sentence was already right. What followed it was a doubled kind, the address
    /// again, and a link to another library's troubleshooting page — inside a World.
    ///
    /// `because` is the measurement that raw text was burying, said in Epoch's own words: empty
    /// where nothing was measured, and never anybody else's sentence. It carries its own leading
    /// separator so a provider with nothing to add renders exactly as it did before.
    #[error("{provider} is not reachable at {endpoint}{because}")]
    Unreachable {
        provider: String,
        endpoint: String,
        because: String,
    },
    #[error("{provider} refused the request: {detail}")]
    Refused { provider: String, detail: String },
    #[error("{provider} answered with something unreadable: {detail}")]
    Unreadable { provider: String, detail: String },
    #[error("no model chosen, and {provider} offers none")]
    NoModel { provider: String },
}

/// What a provider is currently able to do. Always answerable, never an error.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    /// Stable id. What a command addresses; never the display name.
    ///
    /// Owned rather than `&'static str`: a backend is configured now (see [`crate::backends`]),
    /// and two entries of one kind are a thing somebody running Ollama on two machines will
    /// want. A static id could only ever name the kind.
    pub id: String,
    /// What the **program** is called — `Ollama`, `LM Studio`, or whatever the user typed for a
    /// backend they added by hand.
    ///
    /// **Not the computer.** A Bridge used to answer with the machine's name here, because a
    /// machine was one program and the distinction cost nothing. It stopped being free the day a
    /// machine could offer three: the crew editor's `Brain` list read
    /// `studio-mac.local` where it should have read `Ollama`, which is the machine
    /// answering a question about the program.
    pub name: String,
    /// Which computer it runs on, when that is not this one.
    ///
    /// `None` is *this machine*, said once here rather than by every surface deciding what to
    /// call `127.0.0.1`. Derived from the endpoint before this existed — which gave an IP where
    /// a person had already given the machine a name.
    pub machine: Option<String>,
    /// Where we looked. Shown so "OFFLINE" is a fact the user can go and check.
    pub endpoint: String,
    /// True only when the provider actually answered.
    pub online: bool,
    /// True when this provider runs on the user's own machine — no key, no account, no bill.
    pub local: bool,
    /// Models it reports having, in the order it reported them.
    pub models: Vec<String>,
    /// Why it is not online, in plain words. `None` when it is.
    pub note: Option<String>,
}

/// Which computer an endpoint is on, when it is not this one.
///
/// **From the address, because that is the only field that always knows.** A backend's name is
/// whatever the user typed — `the studio box`, `llama.cpp` — and says nothing about where it is.
/// `None` is this machine, decided once here rather than by every surface deciding what to call
/// `127.0.0.1`.
///
/// A Bridge does not use this: it was told the machine's real name at pairing time, and a name
/// a person chose beats a host derived from an IP.
pub fn machine_of(endpoint: &str, local: bool) -> Option<String> {
    if local {
        return None;
    }
    let host = endpoint
        .trim()
        .rsplit("://")
        .next()?
        .split('/')
        .next()?
        .rsplit_once(':')
        .map(|(host, _)| host)
        .unwrap_or(endpoint);
    (!host.is_empty()).then(|| host.to_owned())
}

/// What a backend says one of its models can do.
///
/// **Read, never guessed.** Every backend Epoch talks to publishes this and Epoch read none of
/// it until somebody asked whether an agent could see:
///
/// | asked | answered |
/// |---|---|
/// | Ollama `/api/show` | `capabilities`: `gemma4:12b` → completion, **vision**, **audio**, tools, thinking; `qwen3:14b` → completion, tools, thinking |
/// | Codex app-server | `Model.inputModalities` — `text` / `image` / `audio` |
/// | Claude Code | its first `system` event carries `tools` and `mcp_servers` |
///
/// ADR-0026 already said the Provider declares its surface and the Character holds the value.
/// That was applied to `num_ctx` and to the sliders, and never to what a model *can* — so "does
/// this brain see?" was answered from memory, and from memory it was wrong.
///
/// The fields are the questions Epoch actually acts on. A backend that publishes more says more
/// than this type carries, deliberately: a field nothing reads is a field that goes stale.
/// Re-exported rather than defined here: it crosses a Bridge, so the Kernel owns it (the same
/// reason `HuggingFace` lives there). The doc comment above still describes what it is for.
pub use epoch_kernel::Declared;

impl ProviderStatus {
    fn offline(
        id: impl Into<String>,
        name: impl Into<String>,
        endpoint: String,
        local: bool,
        note: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            machine: machine_of(&endpoint, local),
            endpoint,
            online: false,
            local,
            models: Vec::new(),
            note: Some(note.into()),
        }
    }
}

/// How long a provider should keep a model resident after answering.
///
/// A local model is not free to have loaded: on the machine this was built on, one 14B model
/// held 4 GB of VRAM and stayed there for five minutes after the last word — with the whole
/// crew idle and nothing running. That is somebody's graphics card, and Epoch has no business
/// spending it on a conversation that ended.
///
/// Hosted providers ignore this entirely: there is nothing on the user's machine to release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeepLoaded {
    /// Release as soon as the answer is finished. The next turn pays the load again.
    ///
    /// **The answer, not the round.** A turn with a tool call is several model calls with a
    /// capability running between them, and releasing at each seam meant the model was thrown
    /// out while ComfyUI drew and fully reloaded to say one sentence about the result. Measured
    /// through the window: 213.6 s on llama.cpp and 203.5 s on LM Studio for a picture Ollama
    /// delivered in 84.3 s, and almost all of the difference was load paid twice.
    ///
    /// So a provider keeps the model for [`BETWEEN_ROUNDS`] while a turn is in flight, and
    /// `turn::drive` releases once when the turn ends.
    Never,
    /// Stay resident for a while, so the next turn is instant.
    For(Duration),
}

/// How long a model stays resident *while a turn is still running*, under [`KeepLoaded::Never`].
///
/// Not a policy about how long to hold a card — the explicit release at the end of the turn is
/// that. This is only long enough to cover the **gap between two rounds**, which is a capability
/// running: the longest measured on this machine is a Flux render at 58 s, and this leaves room
/// for one that is slower without holding anything after the turn is over.
///
/// It is a countdown that each request restarts (measured on LM Studio, which is the one server
/// this actually decides for — Ollama and llama.cpp are released outright).
pub const BETWEEN_ROUNDS: Duration = Duration::from_secs(120);

/// One turn's worth of work, in canonical terms.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// Which model to use. A provider-specific string, and the *only* one that crosses this
    /// boundary — chosen by the user from what [`Provider::probe`] reported.
    pub model: String,
    pub conversation: Conversation,
    /// What to do with the model afterwards. The user's call, not ours (see [`KeepLoaded`]).
    pub keep_loaded: KeepLoaded,
    /// Canonical, portable, and this provider's job to translate (ADR-0026).
    pub parameters: Parameters,
    /// Provider-native settings — **only the assigned provider's namespace**, already selected
    /// by the caller. A provider never sees tuning authored for a different one.
    pub tuning: BTreeMap<String, TuningValue>,
    /// What this character may use, this turn.
    ///
    /// Already filtered: the capability exists, the character asked for it, and Trust would not
    /// refuse it outright. A provider renders these into its own tool-calling shape and never
    /// decides what is in the list.
    ///
    /// Empty means no tools are declared at all — not "all of them".
    pub tools: Vec<Descriptor>,
    /// How many rounds of tools this turn may take, when the machine has said.
    ///
    /// ## Why this is settable at all
    ///
    /// `MAX_ROUNDS` is 8, and its reasoning is written down: *enough for look → read → read →
    /// answer, which is the shape of nearly every real request.* That was true, and it was
    /// written before an MCP server with thirty-two tools could be attached to a character. The
    /// owner watched one spend the budget on Spotify searches, announce *"Te lo reproduzco
    /// ahora:"* and stop — correctly, and with nothing left to play it with.
    ///
    /// `None` is the built-in bound and stays the default: **each round is a full model call**,
    /// and a model that has misunderstood loops happily forever. `Some(0)` is no limit at all,
    /// which is the user's to choose on their own machine — STOP is checked before every call,
    /// so an unbounded loop is still one press from ending.
    ///
    /// On `Request` rather than as a parameter, because it is a fact about *this turn* and that
    /// is what this type is. Not a `Parameters` field: it does not survive changing the engine
    /// (ADR-0026's one test) — it is about how patient this installation is.
    pub most_rounds: Option<usize>,
}

/// One thing a model asked to be done.
///
/// The name is validated on arrival, so a model that hallucinates `../etc/passwd` as a tool
/// name is refused here rather than becoming a lookup somewhere downstream.
///
/// The type itself now lives in the Kernel: what a character reached for is part of the
/// exchange, and it has to survive into the Conversation so the result can be paired with it
/// (`epoch_kernel::ToolCall`). This alias is what keeps every existing `provider::ToolCall`
/// reading the same.
pub use epoch_kernel::ToolCall;

/// What one exchange with a provider produced.
///
/// Both halves, because a model may say something *and* ask for a tool in the same breath —
/// "let me check that file" followed by the call. Dropping the words would lose the only
/// explanation the user gets of why it is reaching for something.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Answer {
    pub text: String,
    pub calls: Vec<ToolCall>,
    /// How fast the answer came, **where the backend measured it**.
    ///
    /// ## Why a turn carries this at all
    ///
    /// Epoch measures tokens per second everywhere — the bench, the ladder, QUICK TEST, TIME IT
    /// — and until 2026-09-02 recorded none in the one place a person actually lives with it: a
    /// real conversation. Asked *how fast was that reply?*, the honest answer was to open a
    /// terminal, and the Quest record held nothing.
    ///
    /// **Reported, never computed here.** llama.cpp returns `timings.predicted_per_second` and
    /// Ollama returns `eval_count` over `eval_duration`; both are the server timing its own
    /// decode. A stopwatch around the request would instead measure the model loading, the prompt
    /// being processed and the network, and call the total a generation rate — which is a real
    /// reading of the wrong quantity, and the most convincing way a gauge lies.
    ///
    /// `None` is *the backend did not say*, never zero. A hosted provider that reports no timing
    /// shows nothing rather than a number nobody can defend.
    pub pace: Option<Pace>,
}

/// What a backend said about its own decoding.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pace {
    /// Tokens generated per second, as the backend measured them.
    pub per_second: f64,
    /// How many tokens that was. `None` where the backend gave a rate and not a count.
    #[serde(default)]
    pub generated: Option<u32>,
}

impl Answer {
    /// Whether the model wants to keep working rather than having finished.
    pub fn wants_more(&self) -> bool {
        !self.calls.is_empty()
    }

    /// Take a tool call the model *wrote out* instead of making, and make it.
    ///
    /// ## What was measured
    ///
    /// 2026-08-24, the same question — *"Haz una imagen de Frog de Chrono Trigger"* — put to
    /// `gemma4:12b` through all three local runtimes, with `open_studio` and `draw_image`
    /// declared exactly as Epoch declares them:
    ///
    /// | runtime   | door   | reached for |
    /// |-----------|--------|-------------|
    /// | Ollama    | native | **nothing** |
    /// | llama.cpp | openai | `draw_image` |
    /// | LM Studio | openai | **nothing** |
    ///
    /// Twice out of three it emitted no `tool_calls` at all and wrote this into `content`:
    ///
    /// ```text
    /// { "action": "draw_image", "action_input": "{ \"describe\": \"…Frog…\" }" }
    /// ```
    ///
    /// So the character said a line of JSON in the Chronicle and no picture was drawn. The
    /// runtimes are not the variable — the same model does it on two of them and not the third,
    /// because whether a written call is turned into a real one is the *chat template's* job and
    /// the templates disagree. Epoch cannot fix somebody else's template, and the user should
    /// not have to know which of their three servers happens to have a good one.
    ///
    /// ## Why this is not "repairing the model"
    ///
    /// It is the mirror of [`Answer::hush_echoed_calls`] and it is bounded the same way. The
    /// **whole** message must be one JSON object, a name key must hold the id of a capability
    /// **offered this turn**, and nothing is invented: the arguments are read out of what the
    /// model wrote. A model discussing JSON keeps every word; a model naming a tool it was not
    /// given keeps every word. It can only fire on a message that is nothing but a call this
    /// character could have made.
    ///
    /// The alternative was to leave it, and leaving it means a character on two of this
    /// machine's three runtimes cannot draw at all while appearing to have answered.
    ///
    /// ## What it does not fix
    ///
    /// The written call has already been streamed into the live conversation token by token, so
    /// it flickers before the turn ends — the same cost, and the same reason, as `hush_echoed_calls`.
    pub fn recover_written_call(&mut self, offered: &[Descriptor]) {
        if !self.calls.is_empty() {
            return;
        }
        let text = self.text.trim();
        if !(text.starts_with('{') && text.ends_with('}')) {
            return;
        }
        let Ok(serde_json::Value::Object(said)) = serde_json::from_str(text) else {
            return;
        };
        // The keys a template leaks a call under. Read by name rather than searched for: an
        // object holding a tool name *somewhere* would catch a model talking about one.
        let Some(named) = ["action", "name", "tool", "tool_name", "function"]
            .into_iter()
            .find_map(|key| said.get(key)?.as_str())
        else {
            return;
        };
        if !offered
            .iter()
            .any(|descriptor| descriptor.id.as_str() == named)
        {
            return;
        }
        // Arguments arrive either as an object or as a string holding one, depending on which
        // template leaked. Both were seen in the same afternoon.
        let written = ["action_input", "arguments", "parameters", "input", "args"]
            .into_iter()
            .find_map(|key| said.get(key));
        let arguments = match written {
            Some(serde_json::Value::String(inner)) => {
                serde_json::from_str(inner).unwrap_or(serde_json::json!({}))
            }
            Some(other) => other.clone(),
            None => serde_json::json!({}),
        };
        // Shaped into the wire form and handed to the one reader, rather than mapping values a
        // second time here: two readers of the same thing eventually disagree.
        let Some(call) = read_tool_call(&serde_json::json!({
            "function": { "name": named, "arguments": arguments }
        })) else {
            return;
        };
        self.calls.push(call);
        self.text.clear();
    }

    /// Drop visible text that is only an echo of a tool call this turn already made.
    ///
    /// ## What was measured
    ///
    /// 2026-08-24, `gemma4:12b` through Ollama, asked for a picture. It made a real `tool_calls`
    /// entry **and** wrote the same call into `content`:
    ///
    /// ```text
    /// { "action": "draw_image", "action_input": "{ \"describe\": \"asuka …\" }" }
    /// ```
    ///
    /// The picture was drawn correctly. What landed in the Chronicle was that line, as though
    /// the character had said it.
    ///
    /// ## Why this is not a guess about prose
    ///
    /// It refuses to look at *shape*. The text must parse as JSON **and** name a capability this
    /// turn actually called — a fact Epoch already holds. A model legitimately discussing JSON,
    /// or naming a tool it did not call, keeps every word: the rule can only fire on a duplicate
    /// of something that already happened.
    ///
    /// ## What it does not fix
    ///
    /// Tokens are streamed as they arrive, so the echo may appear for a moment in the live
    /// conversation before the turn ends. Buffering every token until a turn is over to prevent
    /// that would delay every answer to tidy a few. The record is clean; the flicker is not
    /// worth the cost, and saying so is better than implying it never shows.
    pub fn hush_echoed_calls(&mut self) {
        if self.calls.is_empty() {
            return;
        }
        let text = self.text.trim();
        if !text.starts_with('{') {
            return;
        }
        let Ok(said) = serde_json::from_str::<serde_json::Value>(text) else {
            return;
        };
        // The names a leaked template puts a capability under. Read rather than searched for:
        // an object naming a tool anywhere would catch a model *talking* about one.
        let named = ["action", "name", "tool", "tool_name", "function"]
            .into_iter()
            .filter_map(|key| said.get(key)?.as_str())
            .collect::<Vec<_>>();
        if named.iter().any(|name| {
            self.calls
                .iter()
                .any(|call| call.capability.as_str() == *name)
        }) {
            self.text.clear();
        }
    }
}

/// What a provider emits while thinking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chunk {
    /// A fragment of the answer, in order. Never a whole sentence, never guaranteed to be one.
    Token(String),
    /// The provider is done. Emitted exactly once, and only on success.
    Done,
}

/// Who does the thinking.
///
/// Deliberately narrow. Everything a caller needs is "what can you do" and "do this"; anything
/// wider would leak a vendor's shape into the Engine.
/// What a Provider says about one model.
///
/// ## Why the window is not just another control
///
/// Every backend has one and it means the same thing everywhere, so it is **canonical** — the
/// ceiling on `Parameters::context_tokens` (ADR-0026: *measured where possible*). `num_ctx` is
/// Ollama's own name for asking it to use *less* than that ceiling, which is a different thing
/// and belongs in the Provider's own vocabulary.
///
/// Making the interface derive the ceiling from a control called `num_ctx` would teach the
/// Kernel a Provider's parameter name — the exact mistake ADR-0026 was written to prevent.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Surface {
    /// How much this model can hold, if it would say. `None` is *unknown*, never a guess.
    pub window: Option<u32>,
    pub controls: Vec<Control>,
    /// What the backend says this model can **do**. `None` when it would not say.
    ///
    /// Beside `window` because it is the same kind of fact and arrives from the same question:
    /// this is what the model publishes about itself. `window` had been read since ADR-0026 and
    /// this had not, which is how "can this brain see?" came to be answered from memory.
    pub can: Option<Declared>,
}

pub trait Provider: Send + Sync {
    fn id(&self) -> &str;

    /// Ask what this provider can currently do. Never fails — see the module docs.
    fn probe(&self) -> ProviderStatus;

    /// Take one turn, streaming as it goes.
    ///
    /// `sink` is called for every [`Chunk`] in order. The full answer is also returned, so a
    /// caller that only wants the result can ignore the stream — the World wants both: the
    /// tokens to show her thinking, the whole thing to remember what she said.
    fn take_turn(
        &self,
        request: &Request,
        sink: &mut dyn FnMut(Chunk),
    ) -> Result<Answer, ProviderError>;

    /// What this backend says the model can do.
    ///
    /// `None` when the question could not be asked — the backend is not answering, or answered
    /// something unreadable. **Unasked is not "no"**: reporting a model as toolless because a
    /// probe timed out would take its tools away over a network blip, which is the cold-instrument
    /// rule one layer in.
    ///
    /// Declared by the Provider rather than known by the caller (ADR-0003, ADR-0026): a registry
    /// that knew Ollama reports `vision` in `/api/show` would need editing to add a backend that
    /// reports it somewhere else.
    ///
    /// `None` by default, because a Provider that has not implemented this has not been asked.
    fn declares(&self, _model: &str) -> Option<Declared> {
        None
    }

    /// Show one image to one model and return what it says, in words.
    ///
    /// Not a turn: no Conversation, no tools, no streaming, nothing entering a Chronicle. This
    /// is a translation, and `see_image` is what decides whether the answer is worth anything.
    fn describe(&self, _model: &str, _image: &[u8], _look_for: &str) -> Result<String, String> {
        Err("this backend cannot show a picture to a model".into())
    }

    /// What this Provider says about one model.
    ///
    /// Two facts that both belong to the model rather than to the backend, asked together
    /// because asking them separately meant asking the network twice for one answer.
    /// Declared at runtime rather than known by the interface: a frontend that knew Ollama has
    /// `num_ctx` would need editing to add a Provider, and four are coming (ADR-0026, ADR-0003).
    ///
    /// The default says **nothing**, which is the honest answer for a hosted backend whose
    /// whole surface is already the canonical parameters — not an oversight to fill in later.
    ///
    /// Per model, because both facts belong to the model rather than the backend. Allowed to
    /// reach the network for the same reason `probe` is: a measured bound is a fact, and a
    /// constant in our source is a guess that goes stale in silence.
    fn surface(&self, _model: &str) -> Surface {
        Surface::default()
    }

    /// Let go of a model's resources now.
    ///
    /// Only meaningful for providers that hold something on the user's machine. The default is
    /// nothing, which is correct for every hosted provider — there is no memory of theirs to
    /// free, and pretending otherwise would be a call that looks like it did something.
    fn release(&self, _model: &str) {}

    /// Put one model **back** on the card, with nothing to say to it.
    ///
    /// The mirror of [`Provider::release`], and it exists for one moment: a render needs the
    /// whole card, so the model that was resident is let go — and when the picture is finished
    /// the card is free again and the next message should not pay the load.
    ///
    /// **Blocking, and the caller owns that.** Measured on this machine: `gemma4:12b` takes 25 s
    /// to read back. Whoever asks for this is not a turn and is not a person waiting, so it runs
    /// on a thread of its own.
    ///
    /// A no-op by default, which is a real answer and not a gap: only Ollama has been measured
    /// answering a load with no prompt. A backend that has not been asked stays silent rather
    /// than being sent a request written from memory — and the lamp reads the card, so a model
    /// that did not come back says so instead of showing green.
    fn warm(&self, _model: &str) {}

    /// Whether one model is **in memory on this machine right now**, measured.
    ///
    /// The reading behind the lamp on a Chronicle. It is a separate question from anything
    /// Epoch decided: a model Epoch asked to keep can be evicted by the server itself
    /// (llama.cpp's router holds one at a time; LM Studio runs its own idle countdown), and a
    /// model Epoch never loaded can already be resident because somebody opened it in LM
    /// Studio's own window. A lamp wired to the switch instead of to the card would say green
    /// through both.
    ///
    /// `None` is **unasked**, never *no*: a hosted backend holds nothing of the user's to
    /// report, and a local server that will not answer has not said the model is absent. Both
    /// leave the surface with no reading to draw, which is the honest thing to draw.
    ///
    /// Declared by the Provider for the same reason [`Provider::release`] is — each backend
    /// spells this its own way and the caller must not know which is which (ADR-0003).
    fn resident(&self, _model: &str) -> Option<bool> {
        None
    }
}

/// Whether a name a server reported is the model that was asked about.
///
/// Exact, or exact once Ollama's implicit `:latest` is off both sides. A looser match — a
/// prefix, a contains — would light the lamp for `qwen3-14b` when `qwen3-14b-instruct` is what
/// is actually on the card, and the whole point of this reading is that it is not a guess.
pub(crate) fn same_model(reported: &str, asked: &str) -> bool {
    let bare = |name: &str| name.strip_suffix(":latest").unwrap_or(name).to_owned();
    bare(reported) == bare(asked)
}

// ---------------------------------------------------------------------------
// Ollama
// ---------------------------------------------------------------------------

/// Models running on the user's own machine.
///
/// The first Provider, and deliberately so: no API key, no account, no bill, no network. It is
/// the only one that can be true on a laptop with the wifi off, which makes it the honest
/// reference implementation — if the abstraction fits Ollama it fits the rest, and every
/// assumption about credentials had to be someone else's problem from the start.
#[derive(Debug, Clone)]
pub struct Ollama {
    /// Which configured backend this is. Several may exist, on several machines.
    id: String,
    endpoint: String,
    /// Sent as a bearer token when present.
    ///
    /// Ollama itself has no authentication, and on `localhost` this is always `None` — which is
    /// the point of Ollama being the first Provider. But an Ollama reached across a network is
    /// usually behind a reverse proxy that does, and "the machine under the desk" is exactly the
    /// case that made backend id and kind separate questions in the first place.
    ///
    /// It is a [`Secret`], so it cannot be serialised into any projection of this Provider even
    /// by accident (ADR-0026).
    key: Option<Secret>,
}

impl Default for Ollama {
    fn default() -> Self {
        Self::new("http://localhost:11434")
    }
}

/// The same machine, written the way that does not hang.
///
/// ## Measured, because the symptom looks like a dead server
///
/// `localhost` resolves to two addresses and, on Windows, `[::1]` first:
///
/// ```text
/// localhost:11434 -> [[::1]:11434, 127.0.0.1:11434]
/// ```
///
/// Ollama listens on IPv4 only. The v6 attempt does not refuse — it **hangs** — so the fallback
/// only happens when the connection eventually gives up. Timed on this machine, same server, same
/// second:
///
/// ```text
/// http://localhost:11434/api/tags   200 in 15.02s
/// http://127.0.0.1:11434/api/tags   200 in  0.00s
/// ```
///
/// Epoch ships `http://localhost:11434` as Ollama's default, so **every probe of a local Ollama
/// was paying fifteen seconds** — and a caller with a shorter timeout than that read a healthy
/// server as absent, which is how this was found: a five-second read of `/api/tags` returned
/// nothing at all while the same URL answered instantly in a browser.
///
/// ## Why rewriting the address is not guessing
///
/// `localhost` *means* this machine, and `127.0.0.1` is this machine. This is not choosing a
/// different server; it is choosing which of the two addresses of the same one to try, on a
/// platform where one of them is a black hole for IPv4-only listeners. The user's configuration
/// is left exactly as they wrote it — only the URL that goes on the wire changes.
pub fn same_machine(endpoint: &str) -> String {
    endpoint
        .replace("://localhost:", "://127.0.0.1:")
        .replace("://[::1]:", "://127.0.0.1:")
}

impl Ollama {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            id: "ollama".to_owned(),
            endpoint: same_machine(&endpoint.into()),
            key: None,
        }
    }

    /// One configured instance, which may not be the only one.
    ///
    /// The id comes from the configuration rather than from the kind, because "the Ollama on my
    /// laptop" and "the Ollama on the machine under the desk" are two things a character has to
    /// be able to tell apart.
    pub fn named(id: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            endpoint: same_machine(&endpoint.into()),
            key: None,
        }
    }

    /// Give it a bearer token, for an instance reached through something that asks for one.
    pub fn with_key(mut self, key: Option<Secret>) -> Self {
        // An empty key is no key. Sending `Authorization: Bearer ` would turn "I left the field
        // blank" into a refusal from the proxy that reads like the server being down.
        self.key = key.filter(|k| !k.is_empty());
        self
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Add the token, if there is one. Every request goes through here.
    ///
    /// One place rather than four: a request that quietly forgot the header would fail as a 401
    /// on one code path only, which is the kind of thing that gets diagnosed as the model.
    fn signed(&self, request: ureq::Request) -> ureq::Request {
        match &self.key {
            Some(key) => request.set("Authorization", &format!("Bearer {}", key.expose())),
            None => request,
        }
    }
}

impl Ollama {
    /// What this model says it can do.
    ///
    /// **Measured on this machine, not remembered.** `capabilities` holds `vision` for
    /// `gemma4:26b` and `gemma4:12b` and not for `qwen3:14b` or `gpt-oss:20b`, and `audio` for
    /// the 12b alone. So Epoch never needs a "which of my models can see" setting — exactly the
    /// question a person cannot answer from memory, and therefore the wrong one to ask them to
    /// keep correct.
    ///
    /// `None` when it could not be asked. Unreadable is **not** "cannot": a probe that timed out
    /// must not take a model's tools away.
    pub fn declared(&self, model: &str) -> Option<Declared> {
        Self::declared_in(&self.show(model)?)
    }

    /// Show one image to one model and return what it says.
    ///
    /// Deliberately **not** a turn. There is no Conversation, no tools, no streaming and nothing
    /// enters a Chronicle: this is a translation, and the caller
    /// ([`crate::capabilities::see::SeeImage`]) is what decides whether the answer is worth
    /// anything. Routing it through `take_turn` would have made a description into a turn taken
    /// by a character who is not this model.
    ///
    /// The wire shape is measured: `images` is an array of base64 strings on the message itself,
    /// which is Ollama's own arrangement rather than a content-part list.
    pub fn describe(&self, model: &str, image: &[u8], look_for: &str) -> Result<String, String> {
        use base64::Engine as _;
        let encoded = base64::engine::general_purpose::STANDARD.encode(image);

        let answer: serde_json::Value = self
            .signed(
                ureq::builder()
                    .timeout_connect(CONNECT_TIMEOUT)
                    // A vision model on a cold start loads gigabytes before it answers. Generous,
                    // and still finite — the turn timeout's reasoning, one door over.
                    .timeout(TURN_TIMEOUT)
                    .build()
                    .post(&format!("{}/api/chat", self.endpoint)),
            )
            .send_json(serde_json::json!({
                "model": model,
                "stream": false,
                "messages": [{ "role": "user", "content": look_for, "images": [encoded] }],
                // Zero, because this is extraction rather than writing. The same picture and the
                // same question should not give two different answers to two turns.
                "options": { "temperature": 0 },
            }))
            .map_err(|err| format!("{model} could not look at it: {err}"))?
            .into_json()
            .map_err(|err| format!("{model} answered with something unreadable: {err}"))?;

        let said = answer
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .trim();

        if said.is_empty() {
            // Empty is a failure, not a description. A blank string presented as an answer would
            // read to a reasoner as "the image contains nothing".
            return Err(format!("{model} looked at it and said nothing"));
        }
        Ok(said.to_owned())
    }
}

/// Ollama's own wire shape. Confined to this module — see the module docs.
#[derive(Debug, Serialize)]
struct ChatBody<'a> {
    model: &'a str,
    messages: Vec<WireMessage<'a>>,
    stream: bool,
    /// Ollama's own spelling: `0` unloads immediately, `"5m"` keeps it resident.
    keep_alive: String,
    /// Ollama's tuning bag. Canonical parameters are translated into it; provider-native
    /// tuning is passed through verbatim, because these are already Ollama's own words.
    #[serde(skip_serializing_if = "serde_json::Map::is_empty")]
    options: serde_json::Map<String, serde_json::Value>,
    /// Declared only when there are any. An empty array is not the same as absent to every
    /// model, and some refuse to answer at all when handed one.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<serde_json::Value>,
    /// Ollama has a think flag and no gradations. `Off` is false, every other level is true —
    /// the *degree* is discarded rather than faked into a number Ollama does not accept.
    #[serde(skip_serializing_if = "Option::is_none")]
    think: Option<bool>,
}

/// Render a capability into Ollama's tool-calling shape.
///
/// This is where JSON Schema is allowed to exist. The Kernel describes a capability in
/// canonical terms — name, description, [`ValueKind`], required — and every provider turns that
/// into whatever its own API wants, in its own file (ADR-0026, ADR-0008's open question).
///
/// Ollama follows OpenAI's function-calling shape, so this is that.
fn ollama_tool(descriptor: &Descriptor) -> serde_json::Value {
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

/// Read one tool call out of what Ollama sent back.
///
/// Everything here arrives from a **model**, so nothing is assumed: an unknown name is refused
/// by [`CapabilityId::new`], a non-object argument bag yields no arguments rather than a panic,
/// and a value shape we do not model is skipped rather than guessed at.
///
/// A call that cannot be read is dropped and the turn continues. The alternative — failing the
/// whole turn because one of three calls was malformed — throws away work that succeeded.
fn read_tool_call(raw: &serde_json::Value) -> Option<ToolCall> {
    let function = raw.get("function")?;
    let name = function.get("name")?.as_str()?;
    let capability = CapabilityId::new(name).ok()?;

    let mut arguments = Arguments::new();
    if let Some(map) = function.get("arguments").and_then(|a| a.as_object()) {
        for (key, value) in map {
            let held = match value {
                serde_json::Value::String(v) => Value::Text(v.clone()),
                serde_json::Value::Bool(v) => Value::Boolean(*v),
                serde_json::Value::Number(n) if n.is_i64() => Value::Integer(n.as_i64()?),
                serde_json::Value::Number(n) => Value::Number(n.as_f64()?),
                // Null, arrays and objects: no capability declares one, so a model that sent
                // one has misunderstood. Dropping it makes the argument *missing*, which the
                // capability already knows how to report clearly.
                _ => continue,
            };
            arguments = arguments.with(key, held);
        }
    }
    Some(ToolCall {
        capability,
        arguments,
    })
}

/// Where the last turn is written, for when a screenshot is not enough.
///
/// Best effort in every direction: a trace that cannot be written must never cost a turn.
fn trace(body: &ChatBody<'_>) {
    let Ok(pretty) = serde_json::to_string_pretty(body) else {
        return;
    };
    let Some(dir) = std::env::var_os("EPOCH_TRACE_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("last-turn.json"), pretty);
}

/// Translate a character's settings into Ollama's vocabulary.
///
/// Native tuning is applied first and canonical parameters on top, so authoring both
/// `context_tokens` and `num_ctx` resolves to the canonical one. That is the value a surface
/// shows and the one that means the same thing on every backend; letting the backend-specific
/// spelling win would make the visible setting the wrong one.
fn ollama_options(request: &Request) -> serde_json::Map<String, serde_json::Value> {
    use serde_json::Value;

    let mut options = serde_json::Map::new();
    for (key, value) in &request.tuning {
        options.insert(
            key.clone(),
            match value {
                TuningValue::Bool(v) => Value::from(*v),
                TuningValue::Int(v) => Value::from(*v),
                TuningValue::Float(v) => Value::from(*v),
                TuningValue::Text(v) => Value::from(v.clone()),
            },
        );
    }
    if let Some(t) = request.parameters.temperature {
        options.insert("temperature".into(), Value::from(t));
    }
    if let Some(p) = request.parameters.top_p {
        options.insert("top_p".into(), Value::from(p));
    }
    // **How much window this turn actually needs, said out loud.**
    //
    // Ollama's own default is a few thousand tokens and it does not consult the model's
    // reported context length. So a turn carrying a system prompt, a Chronicle and forty-three
    // tool declarations filled the window with the *request* and finished with
    // `done_reason: "length"` before writing a word — which surfaced as "ollama refused the
    // request" and looked like a reasoning problem. Measured on this machine: the model reports
    // 262,144 tokens of context and Epoch was asking for none of it.
    //
    // Asking for the model's maximum instead would be the opposite mistake: 262k of KV cache is
    // memory no consumer graphics card has, and Epoch would trade a truncated turn for one that
    // will not load at all.
    //
    // So: what this turn needs, plus room to answer, rounded up to a step. The steps exist
    // because Ollama reloads the model whenever `num_ctx` changes, and a value that tracked the
    // conversation exactly would reload it on nearly every turn.
    //
    // An explicit `context_tokens` still wins — it is the one lever somebody short on memory
    // has, and a person asking for less has said something Epoch must not overrule.
    let window = match request.parameters.context_tokens {
        Some(asked) => asked,
        None => window_for(request),
    };
    /*
        **And never past the wall this machine was measured at.**

        MEASURE maps a model's curve on a runtime and remembers the largest context that
        actually *loaded* here. Above it the model does not load — it is not a slower answer, it
        is no answer — so a turn that sized itself larger would fail rather than be truncated.
        Capping trades a shortened turn for one that happens, which is the better of the two.

        It only ever lowers, it only applies where somebody ran a curve **on this card and this
        runtime**, and an unmeasured model is not capped at all. A ceiling invented from nothing
        would be the gauge with nothing behind it, applied to a conversation.

        An explicit `context_tokens` is capped too: the wall is a fact about the machine, and a
        number that cannot load is not a preference Epoch can honour.

        **The card is deliberately not re-checked here**, and that is a considered trade rather
        than an oversight: asking which card this is means running `nvidia-smi`, and this runs on
        every single turn. `loadouts.json` lives in this installation's own library, so the only
        way to read a curve from a different card is to have swapped the card — and the worst
        that does is cap a turn lower than the new card needs. A smaller window than necessary is
        recoverable by re-measuring; a probe on every turn is not recoverable at all.
    */
    let window =
        crate::models::loadout::Loadouts::load(crate::models::generative::Library::here().root())
            .ceiling_here(&request.model, "ollama")
            .map_or(window, |most| window.min(most));
    options.insert("num_ctx".into(), Value::from(window));
    options
}

/// Room for this turn and its answer, in tokens, rounded to a step Ollama can keep loaded.
///
/// Reachable from the Bridge as well, deliberately: a turn sent to another machine needs the
/// same window as one taken here, and a second copy of this arithmetic on the far side would be
/// two answers to one question — the far side would also have to be redeployed to change it.
pub(crate) fn window_for(request: &Request) -> u32 {
    /// Characters per token. Deliberately pessimistic: undercounting shrinks the window and
    /// brings back the defect this exists to prevent, while overcounting costs some memory.
    const PER_TOKEN: usize = 3;
    /// Room for the answer itself, on top of everything being sent.
    const TO_ANSWER: usize = 2048;
    /// Reloading is expensive, so the window moves in steps rather than with every message.
    const STEP: usize = 8192;

    let prompt: usize = request
        .conversation
        .messages
        .iter()
        .map(|m| m.content.len())
        .sum();
    // Tool declarations are part of the prompt as far as the model is concerned, and they were
    // the larger half of the turn that failed.
    // Measured as the wire shape, not as the descriptor: what reaches the model is the JSON,
    // and the schema around each name and description is most of its size.
    let tools: usize = request
        .tools
        .iter()
        .map(|tool| ollama_tool(tool).to_string().len())
        .sum();

    let needed = (prompt + tools) / PER_TOKEN + TO_ANSWER;
    let steps = needed.div_ceil(STEP).max(1);
    u32::try_from(steps * STEP).unwrap_or(u32::MAX)
}

/// Ollama's message shape, including the two fields that pair a call with its result.
///
/// **They are not decoration.** Given the call as prose and the result as a `tool` message,
/// `gemma4:12b` answered from imagination; given the same two things in this shape, it answered
/// from the result. See `epoch_kernel::ToolCall` for the measurement.
#[derive(Debug, Serialize)]
struct WireMessage<'a> {
    role: &'static str,
    content: &'a str,
    /// What this message reached for. Ollama's own shape: a function name and an object of
    /// arguments, no ids — the pairing is positional.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tool_calls: Vec<serde_json::Value>,
    /// Which capability produced a `tool` message.
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_name: Option<&'a str>,
}

/// One call, in Ollama's wire shape.
fn ollama_call(call: &ToolCall) -> serde_json::Value {
    serde_json::json!({
        "function": {
            "name": call.capability.as_str(),
            "arguments": call.arguments,
        }
    })
}

impl Ollama {
    /// How much context this model actually has, straight from `/api/show`.
    ///
    /// Ollama reports it as `<family>.context_length` in `model_info`, and the family differs
    /// per model — so the key is found by suffix rather than by a table of families we would
    /// have to keep current. `None` when it cannot be read, and `None` is then *shown* as a
    /// guessed bound rather than quietly replaced by one.
    /// Ask `/api/show` once.
    ///
    /// Both facts Epoch wants about a model — how much it holds, and what it can do — are in this
    /// one answer, and reading it twice was two network round trips every time the editor opened.
    /// The readers below are pure over the result, which is also what makes them testable.
    fn show(&self, model: &str) -> Option<serde_json::Value> {
        let value: serde_json::Value = self
            .signed(
                ureq::builder()
                    .timeout_connect(CONNECT_TIMEOUT)
                    .timeout(PROBE_TIMEOUT)
                    .build()
                    .post(&format!("{}/api/show", self.endpoint)),
            )
            .send_json(serde_json::json!({ "model": model }))
            .ok()?
            .into_json()
            .ok()?;
        Some(value)
    }

    /// How much this model holds, from `/api/show`.
    ///
    /// Ollama reports it as `<family>.context_length` in `model_info`, and the family differs per
    /// model — so the key is found by suffix rather than by a table of families we would have to
    /// keep current.
    fn context_length_in(shown: &serde_json::Value) -> Option<i64> {
        shown
            .get("model_info")?
            .as_object()?
            .iter()
            .find(|(key, _)| key.ends_with(".context_length"))
            .and_then(|(_, v)| v.as_i64())
    }

    /// What this model says it can do, from the same answer.
    fn declared_in(shown: &serde_json::Value) -> Option<Declared> {
        let listed = shown.get("capabilities")?.as_array()?;
        let has = |what: &str| listed.iter().any(|c| c.as_str() == Some(what));
        Some(Declared {
            sees: has("vision"),
            hears: has("audio"),
            // Ollama publishes it per model, so this is a real answer either way.
            uses_tools: Some(has("tools")),
            thinks: has("thinking"),
        })
    }

    /// Ollama's own knobs, given what the model reported.
    fn knobs(&self, measured: Option<i64>) -> Vec<Control> {
        // Measured if it can be, and honest about which it is (ADR-0026). Using a constant as
        // the *ceiling* would cap a 128k model at 32k, which is the kind of quiet wrongness a
        // hardcoded number produces.
        let ceiling = measured.unwrap_or(32_768);

        vec![
            Control {
                name: "num_ctx".into(),
                label: "Context window".into(),
                help: if measured.is_some() {
                    format!("How much this model can hold at once. It reports {ceiling}.")
                } else {
                    "How much this model can hold at once. Ollama did not say what its limit is, so this range is a guess."
                        .into()
                },
                kind: ControlKind::Whole {
                    min: 512,
                    max: ceiling.max(512),
                },
                default: None,
                measured: measured.is_some(),
            },
            Control {
                name: "repeat_penalty".into(),
                label: "Repetition penalty".into(),
                help: "Higher discourages saying the same thing twice. 1.1 is Ollama's default."
                    .into(),
                kind: ControlKind::Ratio { min: 0.5, max: 2.0 },
                default: Some(TuningValue::Float(1.1)),
                measured: false,
            },
            Control {
                name: "top_k".into(),
                label: "Top K".into(),
                help: "How many candidate words are considered each step. Lower is more focused."
                    .into(),
                kind: ControlKind::Whole { min: 1, max: 100 },
                default: Some(TuningValue::Int(40)),
                measured: false,
            },
            Control {
                name: "seed".into(),
                label: "Seed".into(),
                help: "Fix this to get the same answer for the same question. 0 means don't."
                    .into(),
                kind: ControlKind::Whole {
                    min: 0,
                    max: i64::MAX,
                },
                default: Some(TuningValue::Int(0)),
                measured: false,
            },
        ]
    }
}

impl Provider for Ollama {
    fn id(&self) -> &str {
        &self.id
    }

    fn surface(&self, model: &str) -> Surface {
        // One call, two answers. The window is canonical — every backend has one and it means
        // the same thing everywhere — while `num_ctx` is Ollama's own name for asking it to
        // use less than it could.
        let shown = self.show(model);
        let measured = shown.as_ref().and_then(Self::context_length_in);
        Surface {
            window: measured.map(|n| n.max(0) as u32),
            controls: self.knobs(measured),
            can: shown.as_ref().and_then(Self::declared_in),
        }
    }

    fn declares(&self, model: &str) -> Option<Declared> {
        Ollama::declared(self, model)
    }

    fn describe(&self, model: &str, image: &[u8], look_for: &str) -> Result<String, String> {
        Ollama::describe(self, model, image, look_for)
    }

    fn probe(&self) -> ProviderStatus {
        let url = format!("{}/api/tags", self.endpoint);
        let response = self
            .signed(
                ureq::builder()
                    .timeout_connect(CONNECT_TIMEOUT)
                    .timeout(PROBE_TIMEOUT)
                    .build()
                    .get(&url),
            )
            .call();

        let value: serde_json::Value = match response {
            Ok(r) => match r.into_json() {
                Ok(v) => v,
                Err(err) => {
                    return ProviderStatus::offline(
                        self.id(),
                        "Ollama",
                        self.endpoint.clone(),
                        true,
                        format!("answered with something unreadable: {err}"),
                    )
                }
            },
            Err(_) => {
                return ProviderStatus::offline(
                    self.id(),
                    "Ollama",
                    self.endpoint.clone(),
                    true,
                    "not running — start it with `ollama serve`",
                )
            }
        };

        // Ollama reports every pulled model. Order is its own; we do not sort, because "the
        // order it gave" is information and an alphabetical list is not.
        let models: Vec<String> = value["models"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|m| m["name"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();

        ProviderStatus {
            id: self.id().to_owned(),
            // The kind, as a person says it. A configured instance shows its own id beside
            // this, so "Ollama" on two rows is not two mysteries.
            name: "Ollama".to_owned(),
            machine: None,
            endpoint: self.endpoint.clone(),
            online: true,
            local: true,
            note: if models.is_empty() {
                // Running but empty is a real and confusing state. Say what to do about it.
                Some("running, but no models pulled — try `ollama pull qwen3:14b`".into())
            } else {
                None
            },
            models,
        }
    }

    fn take_turn(
        &self,
        request: &Request,
        sink: &mut dyn FnMut(Chunk),
    ) -> Result<Answer, ProviderError> {
        if request.model.trim().is_empty() {
            return Err(ProviderError::NoModel {
                provider: self.id().to_owned(),
            });
        }

        let body = ChatBody {
            model: &request.model,
            messages: request
                .conversation
                .messages
                .iter()
                .map(|m| WireMessage {
                    role: match m.role {
                        Role::System => "system",
                        Role::User => "user",
                        Role::Assistant => "assistant",
                        Role::Tool => "tool",
                    },
                    content: &m.content,
                    tool_calls: m.calls.iter().map(ollama_call).collect(),
                    tool_name: m.tool.as_deref(),
                })
                .collect(),
            stream: true,
            // `0` here would unload the moment *this round* ended, which is a whole model load
            // paid again for every tool call in the turn. `Never` is honoured by `Ollama::release`
            // once the turn is actually over (`turn::drive`); this only has to outlive the gap.
            keep_alive: match request.keep_loaded {
                KeepLoaded::Never => format!("{}s", BETWEEN_ROUNDS.as_secs()),
                KeepLoaded::For(d) => format!("{}s", d.as_secs()),
            },
            options: ollama_options(request),
            tools: request.tools.iter().map(ollama_tool).collect(),
            think: request.parameters.reasoning.map(|r| r != Reasoning::Off),
        };

        // The last turn, written where a person can read it.
        //
        // Added because three rounds of screenshots could not distinguish "the model was never
        // told about its tools" from "the model was told and declined". Both look identical
        // from the outside, and guessing between them wasted more time than writing this.
        //
        // One file, overwritten every turn: a diagnostic, not a log. It lives beside the other
        // vault files because it can contain project text, and that belongs on the user's
        // machine like everything else here.
        trace(&body);

        let url = format!("{}/api/chat", self.endpoint);
        let response = self
            .signed(
                ureq::builder()
                    // Connecting is bounded even though thinking is not: a model may take ten
                    // minutes to answer, but the socket either opens promptly or is not there.
                    .timeout_connect(CONNECT_TIMEOUT)
                    .timeout_read(TURN_TIMEOUT)
                    .build()
                    .post(&url),
            )
            .send_json(&body)
            .map_err(|err| match err {
                ureq::Error::Status(code, r) => ProviderError::Refused {
                    provider: self.id().to_owned(),
                    detail: format!(
                        "{code}: {}",
                        r.into_string().unwrap_or_else(|_| "no detail".into())
                    ),
                },
                ureq::Error::Transport(_) => ProviderError::Unreachable {
                    provider: self.id().to_owned(),
                    endpoint: self.endpoint.clone(),
                    because: String::new(),
                },
            })?;

        // Newline-delimited JSON: one object per token, then one with `done: true`. Read as
        // lines rather than buffered whole, which is what makes the answer arrive *while* it is
        // being written instead of after.
        use std::io::{BufRead, BufReader};
        let mut answer = Answer::default();
        let reader = BufReader::new(response.into_reader());

        for line in reader.lines() {
            let line = line.map_err(|err| ProviderError::Unreadable {
                provider: self.id().to_owned(),
                detail: err.to_string(),
            })?;
            if line.trim().is_empty() {
                continue;
            }

            let frame: serde_json::Value =
                serde_json::from_str(&line).map_err(|err| ProviderError::Unreadable {
                    provider: self.id().to_owned(),
                    detail: err.to_string(),
                })?;

            // Ollama reports a model-level error inside a 200 body, so this is not dead code.
            if let Some(detail) = frame["error"].as_str() {
                return Err(ProviderError::Refused {
                    provider: self.id().to_owned(),
                    detail: detail.to_owned(),
                });
            }

            if let Some(token) = frame["message"]["content"].as_str() {
                if !token.is_empty() {
                    answer.text.push_str(token);
                    sink(Chunk::Token(token.to_owned()));
                }
            }

            // Tool calls arrive whole rather than token by token, and may arrive in any frame
            // — including the final one. Collected across frames because a model may ask for
            // several things at once.
            if let Some(calls) = frame["message"]["tool_calls"].as_array() {
                answer.calls.extend(calls.iter().filter_map(read_tool_call));
            }

            if frame["done"].as_bool() == Some(true) {
                /*
                    **Ollama's own word for the same measurement.** It reports `eval_count`
                    tokens over `eval_duration` nanoseconds — its decode, timed by the server
                    that did it. llama.cpp calls it `timings.predicted_per_second`; neither is
                    computed here, because a rate measured around the request would include the
                    model loading and the prompt being processed and would be a real reading of
                    the wrong quantity.

                    A zero duration is not a fast answer, it is an unmeasured one, and it stays
                    `None`.
                */
                if let (Some(count), Some(ns)) = (
                    frame["eval_count"].as_u64(),
                    frame["eval_duration"].as_u64(),
                ) {
                    if ns > 0 && count > 0 {
                        answer.pace = Some(Pace {
                            per_second: count as f64 / (ns as f64 / 1_000_000_000.0),
                            generated: u32::try_from(count).ok(),
                        });
                    }
                }
                // Two different things end a turn with nothing visible: a reasoning model
                // spending its whole budget in `thinking`, and a turn that did not fit in the
                // window at all. `done_reason` tells them apart, so the sentence does too —
                // the first version blamed reasoning for both, and sent somebody turning a
                // dial that had nothing to do with it.
                if answer.text.is_empty() && answer.calls.is_empty() {
                    let reason = frame["done_reason"].as_str().unwrap_or("no visible answer");
                    return Err(ProviderError::Refused {
                        provider: self.id().to_owned(),
                        detail: format!(
                            "finished without visible content ({reason}); {}",
                            if reason == "length" {
                                "the request filled the model's window before it could answer — fewer capabilities, a shorter conversation, or a larger context_tokens"
                            } else {
                                "this model answered entirely in hidden reasoning — lower its reasoning level"
                            }
                        ),
                    });
                }
                sink(Chunk::Done);
                return Ok(answer);
            }
        }

        // The stream ended without saying it was done. Whatever arrived is still what she said.
        sink(Chunk::Done);
        Ok(answer)
    }

    /// Unload a model from memory now.
    ///
    /// Ollama's documented way to do this is a generate call with `keep_alive: 0` and no
    /// prompt. Best-effort: failing to free memory is not worth failing a turn over, and the
    /// model will fall out on its own timer regardless.
    fn release(&self, model: &str) {
        let _ = self
            .signed(
                ureq::builder()
                    .timeout_connect(CONNECT_TIMEOUT)
                    .timeout(PROBE_TIMEOUT)
                    .build()
                    .post(&format!("{}/api/generate", self.endpoint)),
            )
            .send_json(serde_json::json!({ "model": model, "keep_alive": 0 }));
    }

    /// The same door with the number the other way round, and no prompt.
    ///
    /// **Measured 2026-08-28**, asked of the server rather than remembered: a `/api/generate`
    /// carrying only a model and a `keep_alive` answers
    /// `{"response":"","done":true,"done_reason":"load"}` in 25 s and `/api/ps` then reports the
    /// model resident. Nothing is generated and nothing is charged to a conversation.
    ///
    /// The timeout is its own, because this one is a model load rather than a probe: `PROBE_TIMEOUT`
    /// is for a question a server answers at once, and reading eight gigabytes off disk is not that.
    fn warm(&self, model: &str) {
        let _ = self
            .signed(
                ureq::builder()
                    .timeout_connect(CONNECT_TIMEOUT)
                    .timeout(std::time::Duration::from_secs(300))
                    .build()
                    .post(&format!("{}/api/generate", self.endpoint)),
            )
            .send_json(serde_json::json!({
                "model": model,
                "keep_alive": crate::settings::RESIDENT_MINUTES * 60,
            }));
    }

    /// What Ollama is holding, from `/api/ps`.
    ///
    /// The one runtime where this needs no interpretation: everything `/api/ps` lists is
    /// loaded, and an empty list is the ordinary state between turns. A server that does not
    /// answer yields `None` — it has not told us the model is absent, only that it is not
    /// talking.
    fn resident(&self, model: &str) -> Option<bool> {
        let body: serde_json::Value = self
            .signed(
                ureq::builder()
                    .timeout_connect(CONNECT_TIMEOUT)
                    .timeout(PROBE_TIMEOUT)
                    .build()
                    .get(&format!("{}/api/ps", self.endpoint)),
            )
            .call()
            .ok()?
            .into_json()
            .ok()?;
        let held = body["models"].as_array()?;
        Some(
            held.iter()
                .filter_map(|m| m["name"].as_str())
                .any(|name| same_model(name, model)),
        )
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Every Provider Epoch knows how to speak to.
///
/// Ordered, and the order is the surface's: local first, because a provider that costs nothing
/// and needs no account is the one to try first.
pub struct ProviderRegistry {
    providers: Vec<Box<dyn Provider>>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self {
            providers: vec![Box::new(Ollama::default())],
        }
    }
}

impl ProviderRegistry {
    /// Build one from whatever was configured ([`crate::backends`]).
    pub fn of(providers: Vec<Box<dyn Provider>>) -> Self {
        Self { providers }
    }
}

/// The default registry's ids, for a build with nothing configured.
///
/// It used to be the list a hand-edited character file was validated against, on the reasoning
/// that catching `"ollma"` at load beats failing at the first turn. That reasoning did not
/// survive backends becoming configurable: a backend the user named `desk` is not a typo, and a
/// character carried to a machine where its backend is missing is away from home rather than
/// broken (ADR-0023). Load no longer consults this, and nothing else should either — ask the
/// configured backends, which know what this machine actually has.
pub const DEFAULT_PROVIDERS: [&str; 1] = ["ollama"];

impl ProviderRegistry {
    /// Ask every provider what it can currently do.
    ///
    /*
        **Was sequential, with a comment naming the day it should stop being.** It said
        parallelising becomes worth it "when there are enough providers for the sum to be felt —
        not before", and the sum is being felt: 88.2 s for `who_can_time`, measured twice in the
        window on a machine with the crew's MacBook paired and switched off.

        The sum, not any one probe. Every dead endpoint here costs the same 21 s, measured:
        `192.168.1.20:11500` (a machine that is off) and `127.0.0.1:11434` (Ollama not running) both
        take 21.1 s to fail, because this machine *drops* rather than refuses — the failure mode
        `CONNECT_TIMEOUT` was written for. Six providers, four of them unreachable, one after
        another.

        In parallel the wait is the slowest probe rather than their total, which is the shape the
        answer should have had all along: nothing here depends on anything else here.

        A thread each rather than a pool, because there are six of them and they spend the whole
        time asleep on a socket. A panicking probe loses its provider rather than the survey —
        `probe` is documented never to fail, and a survey that vanished because one implementation
        broke that promise would be the optional last step taking down the job.
    */
    pub fn survey(&self) -> Vec<ProviderStatus> {
        std::thread::scope(|scope| {
            self.providers
                .iter()
                .map(|one| scope.spawn(move || one.probe()))
                .collect::<Vec<_>>()
                .into_iter()
                .zip(self.providers.iter())
                .filter_map(|(asking, one)| match asking.join() {
                    Ok(status) => Some(status),
                    Err(_) => {
                        eprintln!("provider {} panicked while being probed", one.id());
                        None
                    }
                })
                .collect()
        })
    }

    pub fn get(&self, id: &str) -> Option<&dyn Provider> {
        self.providers
            .iter()
            .find(|p| p.id() == id)
            .map(|p| p.as_ref())
    }
}

#[cfg(test)]
mod tests {

    /// `/api/show`, as this machine's Ollama actually answered on 2026-08-16.
    ///
    /// Kept verbatim rather than hand-written, because the point of this parser is that it reads
    /// *their* shape and not the one we would have invented. Two of the three facts below were
    /// unknown until the question was asked.
    #[test]
    fn a_model_s_own_capability_list_is_read_rather_than_remembered() {
        let seeing = serde_json::json!({
            "capabilities": ["completion", "vision", "audio", "tools", "thinking"],
            "model_info": { "gemma4.context_length": 131072 }
        });
        let blind = serde_json::json!({
            "capabilities": ["completion", "tools", "thinking"],
            "model_info": { "qwen3.context_length": 40960 }
        });

        let gemma = Ollama::declared_in(&seeing).expect("it answered");
        assert!(gemma.sees);
        // `gemma4:12b` says audio and `gemma4:26b` does not — a difference nobody knew about
        // until Epoch asked, and one worth showing somebody choosing between them.
        assert!(gemma.hears);
        assert!(gemma.uses_tools == Some(true) && gemma.thinks);

        let qwen = Ollama::declared_in(&blind).expect("it answered");
        assert!(!qwen.sees && !qwen.hears);
        assert!(qwen.uses_tools == Some(true));

        // The same one answer carries the window, which is why it is asked once.
        assert_eq!(Ollama::context_length_in(&seeing), Some(131_072));
    }

    #[test]
    fn a_backend_that_says_nothing_is_unasked_rather_than_incapable() {
        // The distinction the whole type turns on. Reading silence as "no" would take a model's
        // tools away over a network blip, and the symptom — a character that ignores its tools —
        // is among the hardest things to trace back to a timeout.
        let empty = serde_json::json!({ "model_info": { "x.context_length": 8192 } });

        assert_eq!(Ollama::declared_in(&empty), None);
        // And the other fact in the same answer still reads, because they are separate questions.
        assert_eq!(Ollama::context_length_in(&empty), Some(8192));
    }
    use super::*;

    /// An endpoint nothing is listening on. Port 1 needs no privileges to *fail* on.
    fn nowhere() -> Ollama {
        Ollama::new("http://127.0.0.1:1")
    }

    #[test]
    fn a_provider_that_is_not_there_reports_offline_rather_than_failing() {
        // The ordinary state, not an error: nothing in Epoch may claim ONLINE without asking.
        let status = nowhere().probe();
        assert!(!status.online);
        assert!(status.models.is_empty());
        assert!(status.note.is_some(), "offline must always say why");
        assert!(status.local, "Ollama is the user's own machine");
    }

    #[test]
    fn the_lamp_matches_a_model_exactly_or_not_at_all() {
        // A prefix or a `contains` would light this for `qwen3-14b` when what is actually on
        // the card is `qwen3-14b-instruct`. The whole value of this reading is that it is a
        // measurement rather than a guess, so a near miss is a wrong answer.
        assert!(same_model("gemma4:12b", "gemma4:12b"));
        assert!(same_model("qwen3:latest", "qwen3"));
        assert!(same_model("qwen3", "qwen3:latest"));
        assert!(!same_model("qwen3-14b-instruct", "qwen3-14b"));
        assert!(!same_model("gemma4:26b", "gemma4:12b"));
    }

    #[test]
    fn a_backend_that_holds_nothing_of_the_users_says_nothing_rather_than_no() {
        // `None` is unasked. Reading it as "not loaded" is the same invention as reading it as
        // "loaded", and it fails in the direction that merely looks responsible.
        assert_eq!(nowhere().resident("qwen3:14b"), None);
    }

    #[test]
    fn probing_says_where_it_looked() {
        // So "OFFLINE" is a fact the user can go and check, not an opinion.
        let status = nowhere().probe();
        assert_eq!(status.endpoint, "http://127.0.0.1:1");
    }

    #[test]
    fn a_turn_against_nothing_fails_with_the_endpoint_named() {
        let request = Request {
            model: "qwen3:14b".into(),
            conversation: Conversation::opening("be brief"),
            keep_loaded: KeepLoaded::Never,
            parameters: Default::default(),
            tuning: Default::default(),
            tools: Vec::new(),
            most_rounds: None,
        };
        let err = nowhere().take_turn(&request, &mut |_| {}).unwrap_err();
        assert!(matches!(err, ProviderError::Unreachable { .. }));
        assert!(err.to_string().contains("127.0.0.1:1"));
    }

    #[test]
    fn a_turn_with_no_model_is_refused_before_any_request_is_made() {
        let request = Request {
            model: "  ".into(),
            conversation: Conversation::opening(""),
            keep_loaded: KeepLoaded::Never,
            parameters: Default::default(),
            tuning: Default::default(),
            tools: Vec::new(),
            most_rounds: None,
        };
        let err = nowhere().take_turn(&request, &mut |_| {}).unwrap_err();
        assert!(matches!(err, ProviderError::NoModel { .. }));
    }

    #[test]
    fn probing_a_dead_endpoint_returns_promptly() {
        // The regression this exists for: with only a whole-call timeout set, probing a port
        // whose packets are *dropped* rather than refused took 22.5 seconds — the operating
        // system's connect retry schedule, running before the timeout's clock starts. The
        // Launcher asks this on open, so that was a 22-second freeze on any machine whose
        // firewall drops.
        //
        // Bounded generously against a slow CI box; the failure it catches is an order of
        // magnitude away, not a few hundred milliseconds.
        let started = std::time::Instant::now();
        let status = Ollama::default().probe();
        let elapsed = started.elapsed();

        assert!(
            elapsed < Duration::from_secs(5),
            "probing took {elapsed:?}; connect must be bounded, not just the call"
        );
        // Passes whether or not Ollama is actually running on the machine running the test.
        // What is asserted is the *latency*, which is true either way.
        assert_eq!(status.id, "ollama");
    }

    #[test]
    fn every_declared_control_would_accept_its_own_default() {
        // A control whose default its own bounds reject is a contradiction the user meets as a
        // slider that starts outside itself. Checked against a dead endpoint on purpose: this
        // is about the declaration, and `context_length` failing simply means the bound is
        // marked unmeasured.
        let ollama = Ollama::new("http://127.0.0.1:9");
        for control in ollama.surface("whatever:latest").controls {
            let Some(default) = &control.default else {
                continue;
            };
            assert!(
                control.accepts(default),
                "'{}' declares a default its own bounds refuse: {default:?}",
                control.name
            );
        }
    }

    #[test]
    fn an_unmeasured_bound_says_so_rather_than_pretending() {
        // The difference between a fact and a guess has to reach the surface, or a hardcoded
        // number eventually becomes something the user believes the model told us (ADR-0026).
        let ollama = Ollama::new("http://127.0.0.1:9");
        let surface = ollama.surface("whatever:latest");
        let controls = surface.controls;
        let ctx = controls
            .iter()
            .find(|c| c.name == "num_ctx")
            .expect("declared");
        assert!(!ctx.measured);
        assert!(ctx.help.contains("guess"), "{}", ctx.help);
        // And the canonical window is *unknown* rather than guessed, so nothing downstream can
        // present a made-up ceiling as the model's own.
        assert_eq!(surface.window, None);
    }

    #[test]
    fn a_provider_with_nothing_of_its_own_declares_nothing() {
        // The default is none, which is the honest answer for a backend whose whole surface is
        // already the canonical parameters — not an oversight to be filled in later.
        struct Hosted;
        impl Provider for Hosted {
            fn id(&self) -> &'static str {
                "hosted"
            }
            fn probe(&self) -> ProviderStatus {
                ProviderStatus::offline("hosted", "Hosted", String::new(), false, "test")
            }
            fn take_turn(
                &self,
                _request: &Request,
                _sink: &mut dyn FnMut(Chunk),
            ) -> Result<Answer, ProviderError> {
                unreachable!("not asked in this test")
            }
        }
        assert!(Hosted.surface("any").controls.is_empty());
        assert_eq!(Hosted.surface("any").window, None);
    }

    #[test]
    fn the_default_registry_offers_exactly_what_this_build_declares() {
        // The two must not drift. What a *configured* machine offers is a different question,
        // asked of the backends — this is only what exists before anybody configures anything.
        let ids: Vec<String> = ProviderRegistry::default()
            .survey()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(ids, DEFAULT_PROVIDERS.to_vec());
    }

    #[test]
    fn the_registry_offers_the_local_provider_first() {
        // A provider that costs nothing and needs no account is the one to try first.
        let registry = ProviderRegistry::default();
        let survey = registry.survey();
        assert!(!survey.is_empty());
        assert_eq!(survey[0].id, "ollama");
        assert!(survey[0].local);
        assert!(registry.get("ollama").is_some());
        assert!(registry.get("nobody").is_none());
    }

    #[test]
    fn the_engine_never_learns_a_providers_wire_shape() {
        // Executable form of Internal First: a Conversation is built from canonical roles, and
        // the translation to whatever a vendor calls them happens inside the provider.
        let mut c = Conversation::opening("You are precise.");
        c.say(epoch_kernel::Message::user("hello"));

        let body = ChatBody {
            model: "qwen3:14b",
            messages: c
                .messages
                .iter()
                .map(|m| WireMessage {
                    role: m.role.id(),
                    content: &m.content,
                    tool_calls: Vec::new(),
                    tool_name: None,
                })
                .collect(),
            stream: true,
            keep_alive: "0".to_owned(),
            options: Default::default(),
            tools: Vec::new(),
            think: None,
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["messages"][0]["role"], "system");
        assert_eq!(json["messages"][1]["role"], "user");
        assert_eq!(json["stream"], true);
    }

    fn tuned(parameters: Parameters, native: &[(&str, TuningValue)]) -> Request {
        Request {
            model: "qwen3:14b".into(),
            conversation: Conversation::opening(""),
            keep_loaded: KeepLoaded::Never,
            parameters,
            tuning: native
                .iter()
                .map(|(k, v)| ((*k).to_owned(), v.clone()))
                .collect(),
            tools: Vec::new(),
            most_rounds: None,
        }
    }

    #[test]
    fn canonical_parameters_are_translated_into_this_providers_vocabulary() {
        // The Kernel never says `num_ctx`. This is where that word is allowed to exist.
        let options = ollama_options(&tuned(
            Parameters {
                temperature: Some(0.2),
                top_p: Some(0.9),
                context_tokens: Some(32768),
                context_policy: None,
                reasoning: Some(Reasoning::High),
            },
            &[],
        ));
        assert_eq!(options["temperature"], 0.2);
        assert_eq!(options["top_p"], 0.9);
        assert_eq!(options["num_ctx"], 32768);
        // Reasoning is not an option — it is a flag, and it is applied on the body.
        assert!(!options.contains_key("reasoning"));
    }

    #[test]
    fn nothing_unset_is_sent_so_the_providers_own_defaults_stand() {
        let options = ollama_options(&tuned(Parameters::default(), &[]));

        // Everything the character did not choose stays unsaid — except the window, which is
        // not a preference but a fact about whether the turn fits at all. See below.
        assert!(!options.contains_key("temperature"), "{options:?}");
        assert!(!options.contains_key("top_p"), "{options:?}");
        assert_eq!(options.keys().collect::<Vec<_>>(), ["num_ctx"]);
    }

    #[test]
    fn the_window_asked_for_covers_the_turn_being_sent() {
        // The defect: Ollama's default window is a few thousand tokens and ignores what the
        // model reports, so a turn carrying a long prompt and forty-three tool declarations
        // filled the window with the request and finished with `done_reason: "length"` before
        // writing a word. Measured on the machine this was written against: the model reports
        // 262,144 tokens of context, and Epoch was asking for none of it.
        let mut request = tuned(Parameters::default(), &[]);
        request
            .conversation
            .say(epoch_kernel::Message::user("x".repeat(60_000)));
        let modest = ollama_options(&request)["num_ctx"].as_u64().unwrap();
        assert!(
            modest >= 60_000 / 3,
            "a 60k-character prompt needs room for itself: {modest}"
        );

        // Tools are part of the prompt as far as the model is concerned, and they were the
        // larger half of the turn that failed.
        request.tools = (0..40)
            .map(|n| {
                Descriptor::observing(
                    epoch_kernel::CapabilityId::new(&format!("tool_{n}")).unwrap(),
                    "reads something, and says so at the length a real summary runs to",
                )
            })
            .collect();
        let with_tools = ollama_options(&request)["num_ctx"].as_u64().unwrap();
        assert!(
            with_tools > modest,
            "declaring tools must widen the window: {with_tools} vs {modest}"
        );

        // And a character who asked for less still gets less: that is the one lever somebody
        // short on memory has, and Epoch must not overrule it.
        let asked = ollama_options(&tuned(
            Parameters {
                context_tokens: Some(4096),
                ..Parameters::default()
            },
            &[],
        ));
        assert_eq!(asked["num_ctx"], 4096);
    }

    #[test]
    fn provider_native_tuning_passes_through_untouched() {
        let options = ollama_options(&tuned(
            Parameters::default(),
            &[
                ("repeat_penalty", TuningValue::Float(1.1)),
                ("numa", TuningValue::Bool(true)),
            ],
        ));
        assert_eq!(options["repeat_penalty"], 1.1);
        assert_eq!(options["numa"], true);
    }

    #[test]
    fn a_canonical_parameter_wins_over_the_backends_own_spelling_for_it() {
        // Authoring both is a mistake, but a silent one either way. The canonical value wins
        // because it is the one a surface shows and the one that means the same thing on every
        // backend — losing to the local spelling would make the visible setting the wrong one.
        let options = ollama_options(&tuned(
            Parameters {
                context_tokens: Some(32768),
                ..Parameters::default()
            },
            &[("num_ctx", TuningValue::Int(2048))],
        ));
        assert_eq!(options["num_ctx"], 32768);
    }

    #[test]
    fn reasoning_becomes_a_flag_this_backend_can_actually_honour() {
        // Ollama has no gradations. Off is false, everything above it is true, and the degree
        // is discarded rather than invented into a number Ollama would reject.
        let flag = |r: Option<Reasoning>| -> Option<bool> { r.map(|r| r != Reasoning::Off) };
        assert_eq!(flag(None), None, "unset must not send the flag at all");
        assert_eq!(flag(Some(Reasoning::Off)), Some(false));
        assert_eq!(flag(Some(Reasoning::Low)), Some(true));
        assert_eq!(flag(Some(Reasoning::Max)), Some(true));
    }
}

#[cfg(test)]
mod an_echoed_call {
    use super::*;
    use epoch_kernel::{Arguments, CapabilityId};

    fn calling(name: &str) -> Vec<epoch_kernel::ToolCall> {
        vec![epoch_kernel::ToolCall {
            capability: CapabilityId::new(name).expect("a name"),
            arguments: Arguments::new(),
        }]
    }

    #[test]
    fn a_model_that_wrote_its_own_call_said_nothing() {
        // Measured 2026-08-24: `gemma4:12b` made a real tool call *and* wrote the same call into
        // its answer, so the Chronicle recorded that line as though the character had said it.
        let mut answer = Answer {
            text: r#"{ "action": "draw_image", "action_input": "{\"describe\": \"asuka\"}" }"#
                .to_owned(),
            calls: calling("draw_image"),
            pace: None,
        };
        answer.hush_echoed_calls();
        assert_eq!(answer.text, "");
        // The call itself is untouched: it is the thing that actually happened.
        assert_eq!(answer.calls.len(), 1);
    }

    #[test]
    fn a_model_talking_about_json_keeps_every_word() {
        // The rule may only fire on a duplicate of something that already happened. Shape alone
        // is never enough, or a character explaining a payload would be silenced.
        let mut answer = Answer {
            text: r#"{ "action": "something_else", "why": "an example for you" }"#.to_owned(),
            calls: calling("draw_image"),
            pace: None,
        };
        answer.hush_echoed_calls();
        assert!(answer.text.starts_with('{'), "{}", answer.text);
    }

    #[test]
    fn a_turn_with_no_calls_is_left_alone() {
        // Without a call there is nothing it could be a duplicate of.
        let mut answer = Answer {
            text: r#"{ "action": "draw_image" }"#.to_owned(),
            calls: Vec::new(),
            pace: None,
        };
        answer.hush_echoed_calls();
        assert!(!answer.text.is_empty());
    }

    #[test]
    fn ordinary_prose_is_never_touched() {
        let mut answer = Answer {
            text: "Listo, aqui tienes la imagen.".to_owned(),
            calls: calling("draw_image"),
            pace: None,
        };
        answer.hush_echoed_calls();
        assert_eq!(answer.text, "Listo, aqui tienes la imagen.");
    }
}

#[cfg(test)]
mod a_written_call {
    use super::*;

    fn offering(name: &str) -> Vec<Descriptor> {
        vec![Descriptor::acting(
            epoch_kernel::CapabilityId::new(name).expect("a name"),
            "whatever it does",
            [epoch_kernel::Effect::Reads],
            epoch_kernel::Reversal::NothingToUndo,
        )]
    }

    #[test]
    fn a_call_the_model_only_wrote_down_is_made() {
        // Measured 2026-08-24 on Ollama and LM Studio: `gemma4:12b` emitted no `tool_calls` and
        // wrote this instead, so the character said a line of JSON and drew nothing.
        let mut answer = Answer {
            text: r#"{ "action": "draw_image", "action_input": "{\"describe\": \"Frog\"}" }"#
                .to_owned(),
            calls: Vec::new(),
            pace: None,
        };
        answer.recover_written_call(&offering("draw_image"));
        assert_eq!(answer.calls.len(), 1, "{answer:?}");
        assert_eq!(answer.calls[0].capability.as_str(), "draw_image");
        assert_eq!(
            answer.calls[0]
                .arguments
                .text("describe")
                .unwrap_or_default(),
            "Frog"
        );
        // And the JSON stops being something the character said.
        assert_eq!(answer.text, "");
    }

    #[test]
    fn arguments_written_as_an_object_read_the_same() {
        // The other shape seen the same afternoon. Two templates, one meaning.
        let mut answer = Answer {
            text: r#"{"name": "draw_image", "arguments": {"describe": "Frog"}}"#.to_owned(),
            calls: Vec::new(),
            pace: None,
        };
        answer.recover_written_call(&offering("draw_image"));
        assert_eq!(answer.calls.len(), 1, "{answer:?}");
        assert_eq!(
            answer.calls[0]
                .arguments
                .text("describe")
                .unwrap_or_default(),
            "Frog"
        );
    }

    #[test]
    fn a_tool_this_character_was_not_given_is_never_invented() {
        // The guard that keeps this from being a JSON-shaped free-for-all: the name must be
        // something Epoch actually offered this turn.
        let mut answer = Answer {
            text: r#"{ "action": "delete_everything", "action_input": "{}" }"#.to_owned(),
            calls: Vec::new(),
            pace: None,
        };
        answer.recover_written_call(&offering("draw_image"));
        assert!(answer.calls.is_empty());
        assert!(answer.text.starts_with('{'), "{}", answer.text);
    }

    #[test]
    fn a_real_call_is_never_second_guessed() {
        // With a call already made there is nothing to recover, and overwriting it would replace
        // what happened with a reading of what was typed.
        let mut answer = Answer {
            text: r#"{ "action": "draw_image", "action_input": "{}" }"#.to_owned(),
            calls: vec![epoch_kernel::ToolCall {
                capability: epoch_kernel::CapabilityId::new("open_studio").expect("a name"),
                arguments: epoch_kernel::Arguments::new(),
            }],
            pace: None,
        };
        answer.recover_written_call(&offering("draw_image"));
        assert_eq!(answer.calls.len(), 1);
        assert_eq!(answer.calls[0].capability.as_str(), "open_studio");
    }

    #[test]
    fn prose_wrapped_around_json_is_left_alone() {
        // The whole message must be the call. A sentence with an example in it is a sentence.
        let mut answer = Answer {
            text: r#"You could send { "action": "draw_image" } to do that."#.to_owned(),
            calls: Vec::new(),
            pace: None,
        };
        answer.recover_written_call(&offering("draw_image"));
        assert!(answer.calls.is_empty());
        assert!(answer.text.starts_with("You could"));
    }
}
