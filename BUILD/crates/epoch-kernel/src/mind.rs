//! Who does a character's thinking, and how (ADR-0026).
//!
//! ## One test decides where a setting lives
//!
//! > **Does this survive changing the engine?**
//!
//! Yes → a [`Parameters`] field: a small closed set, canonical, meaningful to any backend
//! present or future. These are *behavioural identity* — a Guardian at `0.2` and a Researcher
//! at `0.9` differ in who they are, not in what hardware they run on — so they must survive
//! moving a character from a local runtime to a hosted one. Each Provider translates them into
//! its own API.
//!
//! No → [`Tuning`]: an open set, namespaced by provider, opaque to the Kernel. `num_ctx` is an
//! *Ollama parameter name*; it does not exist in Claude and is called something else again in
//! LM Studio. Putting it here as a canonical field would smuggle a provider into the Kernel —
//! the same mistake as putting `num_gpu` on a character, one step later.
//!
//! Resource and connection settings (`num_gpu`, `num_thread`, `use_mmap`, endpoint,
//! credentials) are neither: they belong to a machine rather than to a person, and live on the
//! Provider. That split is what makes a shared character *structurally incapable* of carrying
//! an API key — the field does not exist on this type.
//!
//! ## Nothing here is defaulted
//!
//! Every parameter is optional, and unset means *the Provider's own default* rather than a
//! number we invented. A `temperature = 0.7` that Epoch made up would be indistinguishable, in
//! the file and in the UI, from one the user chose.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// How much a character deliberates before answering.
///
/// Canonical because it appeared independently on both sides of the divide: a local runtime has
/// a think flag, a hosted provider has effort levels. The concept crosses backends and it is
/// character identity — a Guardian deliberates more than a Researcher, on any engine.
///
/// The **ladder** is canonical; each Provider maps it onto whatever its API actually offers,
/// read from that API rather than assumed. A Provider with nothing to map it to ignores it and
/// says so, rather than pretending it applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Reasoning {
    Off,
    Low,
    Medium,
    High,
    /// Between High and Max.
    ///
    /// Added because a backend actually has it — `claude --effort low|medium|high|xhigh|max`,
    /// read from the program rather than remembered. Without this rung the ladder could not
    /// *reach* a level that exists, which is a worse failure than carrying one somebody maps
    /// down: a canonical ladder is allowed to be finer than a backend, never coarser than all
    /// of them.
    ///
    /// It is still the concept, not the vendor. Granularity differs everywhere — one runtime has
    /// a boolean, another has four levels, another five — so a rung nobody can honour maps down
    /// and says so, exactly as `Off` already does.
    XHigh,
    Max,
}

impl Reasoning {
    pub const ALL: [Reasoning; 6] = [
        Reasoning::Off,
        Reasoning::Low,
        Reasoning::Medium,
        Reasoning::High,
        Reasoning::XHigh,
        Reasoning::Max,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Reasoning::Off => "off",
            Reasoning::Low => "low",
            Reasoning::Medium => "medium",
            Reasoning::High => "high",
            Reasoning::XHigh => "xhigh",
            Reasoning::Max => "max",
        }
    }

    /// What to call this rung on screen.
    ///
    /// Here rather than in a surface because every surface would otherwise invent its own words
    /// for the same ladder, and two surfaces disagreeing about what `xhigh` is called is the
    /// same class of bug as two stores disagreeing about a value.
    pub fn label(self) -> &'static str {
        match self {
            Reasoning::Off => "Off",
            Reasoning::Low => "Low",
            Reasoning::Medium => "Medium",
            Reasoning::High => "High",
            Reasoning::XHigh => "xHigh",
            Reasoning::Max => "Max",
        }
    }

    /// Accept a choice arriving from a surface, or refuse it.
    ///
    /// A surface may not hand the Engine a value it invented — the same reason
    /// `CharacterArchetype::from_id` exists.
    pub fn from_id(raw: &str) -> Option<Self> {
        Reasoning::ALL
            .into_iter()
            .find(|r| r.as_str() == raw.trim().to_ascii_lowercase())
    }
}

impl std::fmt::Display for Reasoning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Canonical parameters. Small, closed, and provider-independent.
///
/// Growth is the signal that the boundary is being applied wrongly: anything backend-specific
/// belongs in [`Tuning`], not here.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Parameters {
    /// How much the character improvises.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// The same, by a different mechanism.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    /// How much history this character is asked to hold.
    ///
    /// ## Superseded by `context_policy` (ADR-0026 amendment, 2026-08-31)
    ///
    /// The ADR's own open question doubted this one, and building the Models deck settled it: a
    /// window is not a property of a character. `llama-server` takes `--ctx-size` when it spawns
    /// the child that holds the model, so **two characters on one model physically cannot have
    /// different windows** — a per-character number there is unsatisfiable rather than awkward.
    /// And `131072` depends on the model, the artefact, the card, the runtime and the machine, so
    /// carrying it to a smaller one names a quantity nobody can honour.
    ///
    /// **Kept, and not written by any surface.** A character file that already carries it still
    /// loads, and the value is still honoured as a ceiling where a Provider can take one — a
    /// character asking to hold *less* than the window is a request the Composer can satisfy on
    /// any backend. What no longer happens is a surface offering it, or Epoch treating it as the
    /// window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_tokens: Option<u32>,
    /// How this character wants to use whatever window it is given.
    ///
    /// ## The half of the old `context_tokens` that is genuinely identity
    ///
    /// Not *how much window* — that is decided once, in MODELS, for the loaded model, and every
    /// character on that model inherits it. This is *how to fill the window there is*, which is
    /// the part that describes the character and survives moving it to another machine.
    ///
    /// A Historian who wants everything it can have still wants that on a bigger card. A Guardian
    /// that works from little still does. Neither statement is a number, and both are true
    /// wherever they are carried (ADR-0023).
    ///
    /// **The policy governs the space between the floor and the ceiling, never the floor.** The
    /// domain rule, from the amendment:
    ///
    /// ```text
    /// Required Context  ≤  Character Effective Use  ≤  Model Context Window
    /// ```
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_policy: Option<ContextPolicy>,
    /// How much they deliberate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<Reasoning>,
}

impl Parameters {
    /// Whether the author set anything at all.
    pub fn is_empty(&self) -> bool {
        *self == Parameters::default()
    }

    /// Every reason this set is unusable, or none.
    ///
    /// Refuses rather than clamps. A silently corrected `temperature = 7.0` would leave the
    /// file saying one thing and the model doing another, which is the failure this whole ADR
    /// exists to prevent.
    pub fn problems(&self) -> Vec<String> {
        let mut found = Vec::new();
        if let Some(t) = self.temperature {
            if !(0.0..=2.0).contains(&t) || !t.is_finite() {
                found.push(format!("temperature {t} is outside 0.0..=2.0"));
            }
        }
        if let Some(p) = self.top_p {
            if !(0.0..=1.0).contains(&p) || !p.is_finite() {
                found.push(format!("top_p {p} is outside 0.0..=1.0"));
            }
        }
        if let Some(c) = self.context_tokens {
            // Below this nothing survives composition — not even one system message plus one
            // question. A number that cannot produce a turn is a typo, not a preference.
            if c < 256 {
                found.push(format!(
                    "context_tokens {c} is too small to hold a turn (minimum 256)"
                ));
            }
        }
        found
    }
}

/// How a character wants to use the window it is given.
///
/// **A policy, never a size.** Four intentions that mean the same thing on any machine, which is
/// what a portable character can carry (ADR-0026 amendment). The numbers they turn into are
/// computed from the window that is actually there, and the floor is never negotiable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContextPolicy {
    /// Required, and a little of what is optional. A character that works from little.
    Compact,
    /// Required, and as much of the optional space as the turn needs.
    ///
    /// **`Adaptive`, not `Auto`.** MODELS keeps `AUTO` for choosing a physical profile, and these
    /// are two decisions in two layers. One word meaning two things has cost this repository a
    /// dead `Manual` mode, a confusion between `-ngl` and `-ot`, and a whole search run without
    /// the one technique it existed to measure.
    #[default]
    Adaptive,
    /// Required, and as much of the optional space as is available.
    Long,
    /// An explicit policy the author has configured. What that means is theirs.
    Custom,
}

impl ContextPolicy {
    /// What share of the *optional* capacity this asks for.
    ///
    /// **Of the optional capacity, never of the window.** `window − required` is what these
    /// divide; the required blocks are already spent before any of this is asked (ADR-0012 forbids
    /// dropping them).
    ///
    /// `Adaptive` returns `None`: it is not a fraction, it is *the Composer decides from the turn*,
    /// and returning a number here would be inventing the answer it exists to work out. Same for
    /// `Custom`, whose answer is the author's.
    ///
    /// The two fractions are a starting point rather than a measurement, and they are the only
    /// numbers in this file that were chosen rather than read. They are named here so there is
    /// one place to correct them once there is something to correct them against.
    pub fn share_of_optional(self) -> Option<f64> {
        match self {
            ContextPolicy::Compact => Some(0.25),
            ContextPolicy::Long => Some(0.90),
            ContextPolicy::Adaptive | ContextPolicy::Custom => None,
        }
    }

    /// Every policy, in the order somebody reads them.
    pub const ALL: [ContextPolicy; 4] = [
        ContextPolicy::Adaptive,
        ContextPolicy::Compact,
        ContextPolicy::Long,
        ContextPolicy::Custom,
    ];

    /// What a file writes, and the only thing that parses one back.
    pub fn id(self) -> String {
        self.name().to_lowercase()
    }

    /// Read a policy from what a file or a surface wrote. `None` for a word this build does not
    /// know — a caller refuses rather than defaulting, because a file saying `"lnog"` would
    /// otherwise load as `Adaptive` and behave as something nobody asked for.
    ///
    /// **Derived from `name`, never from a second table.** A parse table beside a print table is
    /// one fact written twice, and the half nobody is watching is the half where the two
    /// spellings differ — which in this repository has already cost a dark AUDIO tab.
    pub fn from_id(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|it| it.id() == raw.trim())
    }

    /// What a surface calls it.
    pub fn name(self) -> &'static str {
        match self {
            ContextPolicy::Compact => "Compact",
            ContextPolicy::Adaptive => "Adaptive",
            ContextPolicy::Long => "Long",
            ContextPolicy::Custom => "Custom",
        }
    }

    /// What it is for, in one line.
    pub fn about(self) -> &'static str {
        match self {
            ContextPolicy::Compact => "Works from little. Keeps the turn short.",
            ContextPolicy::Adaptive => "Uses as much as the task needs.",
            ContextPolicy::Long => "Uses as much of the window as is available.",
            ContextPolicy::Custom => "Configured by you.",
        }
    }
}

/// How much of a window a character may actually use.
///
/// ## The domain rule, computed
///
/// ```text
/// Required Context  ≤  Character Effective Use  ≤  Model Context Window
/// ```
///
/// **The floor wins.** A `Compact` preference of 8K on a character whose required blocks cost 12K
/// produces a 12K character, not an 8K one — the policy divides `window − required`, and required
/// is spent before it is asked.
///
/// `None` where the character cannot run at all: required is larger than the window. That is a
/// **configuration error to be found when a profile is applied**, not a turn that fails.
pub fn effective_use(
    window: u32,
    required: u32,
    policy: ContextPolicy,
    asked_for: Option<u32>,
) -> Option<u32> {
    if required > window {
        return None;
    }
    let optional = window - required;
    let allowed = match policy.share_of_optional() {
        // A fraction of what is optional, on top of the floor.
        Some(share) => required + (optional as f64 * share) as u32,
        // Adaptive and Custom take the whole window as their ceiling; what they actually use is
        // decided per turn by the Composer, which is the point of them.
        None => window,
    };
    // A character that asked to hold *less* than that still holds less. This is the surviving
    // half of `context_tokens`: a ceiling a character may lower, never one it may raise.
    let capped = asked_for.map_or(allowed, |it| allowed.min(it.max(required)));
    Some(capped.clamp(required, window))
}

/// One value in a Provider's own vocabulary.
///
/// Deliberately *not* `toml::Value` or `serde_json::Value`: the Kernel does not know what a
/// serialisation format is, and a character authored in TOML is shared as JSON (ADR-0026). A
/// small scalar set round-trips cleanly through both.
///
/// Flat scalars only. A Provider needing nested structure has outgrown "tuning" and should
/// declare a canonical parameter or a control of its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TuningValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
}

impl std::fmt::Display for TuningValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TuningValue::Bool(v) => write!(f, "{v}"),
            TuningValue::Int(v) => write!(f, "{v}"),
            TuningValue::Float(v) => write!(f, "{v}"),
            TuningValue::Text(v) => f.write_str(v),
        }
    }
}

/// One knob a Provider says it has.
///
/// ## Why the Provider declares this instead of the interface knowing it
///
/// A frontend that knew Ollama has `num_ctx` would need editing to add a Provider — and ADR-0003
/// says presentation holds no logic, while ADR-0026 says the Provider declares the surface and
/// the Character holds the value. Four more Providers are coming. Each one teaching the UI
/// about itself is four edits to a file that should never have heard of any of them.
///
/// So a control is **data**: a name, a shape, bounds, a default and a sentence. Any surface can
/// draw a slider from that without knowing which backend asked for it, and a local model can
/// expose a large surface while a hosted one exposes three, without either being flattened into
/// a shared schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Control {
    /// The Provider's own name for it — `num_ctx`, `repeat_penalty`. This is the key it will be
    /// stored under in [`Tuning`], so it is that vocabulary and not a translated one.
    pub name: String,
    /// What a person reads. The native name is precise and frequently unreadable.
    pub label: String,
    /// One line on what it does, in the words of somebody who has not read the API docs.
    pub help: String,
    pub kind: ControlKind,
    /// What the Provider does when nobody sets it. `None` when even the Provider cannot say.
    ///
    /// Shown rather than pre-filled: a value the user never chose should not be written into
    /// their character's file, where it would look like a decision.
    pub default: Option<TuningValue>,
    /// True when the bound came from asking rather than from a constant in our source.
    ///
    /// ADR-0026: *measured where possible*. A context limit read from the model is a fact; the
    /// same number hardcoded is a guess that goes stale silently, and a surface should be able
    /// to tell the user which one it is looking at.
    #[serde(default)]
    pub measured: bool,
}

/// What shape a control is, and therefore what a surface should draw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ControlKind {
    /// On or off.
    Toggle,
    /// A whole number between two bounds. Drawn as a slider, because a number with a known
    /// range that is typed into a box is a number somebody will eventually type wrongly.
    Whole { min: i64, max: i64 },
    /// A fraction between two bounds.
    Ratio { min: f64, max: f64 },
    /// One of a fixed set.
    Choice { options: Vec<String> },
    /// Free text, for the few things that genuinely are.
    Free,
}

impl Control {
    /// Whether a value could belong to this control.
    ///
    /// Checked in the Engine rather than trusted from a surface: a surface's validation is a
    /// courtesy, never a control (ADR-0024, arrived at the same way).
    pub fn accepts(&self, value: &TuningValue) -> bool {
        match (&self.kind, value) {
            (ControlKind::Toggle, TuningValue::Bool(_)) => true,
            (ControlKind::Whole { min, max }, TuningValue::Int(v)) => v >= min && v <= max,
            (ControlKind::Ratio { min, max }, TuningValue::Float(v)) => v >= min && v <= max,
            // A whole number is a perfectly good ratio, and a model that emits `1` for a float
            // control is not making a mistake worth refusing.
            (ControlKind::Ratio { min, max }, TuningValue::Int(v)) => {
                let v = *v as f64;
                v >= *min && v <= *max
            }
            (ControlKind::Choice { options }, TuningValue::Text(v)) => options.contains(v),
            (ControlKind::Free, TuningValue::Text(_)) => true,
            _ => false,
        }
    }
}

/// Provider-native settings, namespaced by provider id.
///
/// The Kernel never interprets these; they are opaque to everything except the Provider that
/// declared them. Unknown keys are **preserved**, so a character authored against a newer
/// Provider is not destroyed by an older Epoch reading and rewriting the file.
pub type Tuning = BTreeMap<String, BTreeMap<String, TuningValue>>;

/// Something a character asks to be able to use.
///
/// **Intent, never a claim.** `vision` in a file does not give a blind model sight; only the
/// resolved Provider can say what is actually available (ADR-0005 — capability is not
/// persistent; ADR-0011 — *Requested* Capabilities).
///
/// Validated for shape but not against a fixed list: the Phase-1 capability taxonomy is still
/// to be designed, and a closed enum today would pre-empt that decision with a guess. Earn
/// Complexity — it narrows the day the real set exists.
///
/// ## Two shapes: one capability, or one source of them
///
/// `read_file` names a capability. `mcp:playwright` names a **source** — everything one thing
/// offers, whatever that turns out to be at the moment the request is resolved.
///
/// The second shape exists because the first cannot stay true. A connected MCP server can offer
/// two dozen tools and can offer a different two dozen tomorrow; a file listing today's ids is
/// frozen the moment it is written, and nothing would tell the user that a tool added later is
/// missing. A request that names the source is written once and is never out of date.
///
/// The Kernel validates the shape and **never interprets the scope**. `mcp` means nothing here,
/// exactly as a provider namespace in [`Tuning`] means nothing here: what a scope resolves to is
/// an Engine question, because only the Engine can see what is connected.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct CapabilityRequest(String);

impl CapabilityRequest {
    // `INTENTIONS` lived here: capabilities worth asking for that nothing could do yet, and it
    // held exactly one — `vision`.
    //
    // **Removed 2026-08-16, because sight stopped being something to declare.** It is measured
    // now: every backend publishes what its models can do, and Epoch reads it
    // (`provider::Declared`). A model either sees or it does not, and a tick-box asking a person
    // to assert it was a control that governed nothing — the gauge that lies, in the one place
    // the user is told they are choosing what somebody can do.
    //
    // Reaching for a picture through a *translator* is a different thing and it keeps its own
    // id, `see_image`, in the live catalogue like every other capability.
    //
    // The concept is sound and may return the day there is a real instance of it. An empty array
    // nobody fills is not that day. A character file still carrying `vision` is unharmed: an
    // unrecognised request is preserved and shown rather than rewritten (ADR-0026).

    pub fn new(raw: &str) -> Result<Self, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("a requested capability cannot be empty".into());
        }
        // At most one `:`, and both halves present. `mcp:` and `:playwright` are refused rather
        // than stored, because a half-written scope resolves to nothing and would look like a
        // capability that simply never arrives.
        let mut halves = trimmed.split(':');
        let (first, second) = (halves.next().unwrap_or_default(), halves.next());
        if halves.next().is_some() {
            return Err(format!(
                "requested capability '{trimmed}' has more than one ':'; a scope is 'source:name'"
            ));
        }
        for half in [Some(first), second].into_iter().flatten() {
            if half.is_empty() {
                return Err(format!(
                    "requested capability '{trimmed}' has an empty half; a scope is 'source:name'"
                ));
            }
            if let Some(bad) = half
                .chars()
                .find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_'))
            {
                return Err(format!(
                    "requested capability '{trimmed}' contains '{bad}'; use lowercase letters, digits or '_'"
                ));
            }
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The source and the name, when this request names a whole source rather than one thing.
    ///
    /// `None` for `read_file`. `Some(("mcp", "playwright"))` for `mcp:playwright`. What either
    /// half means is not decided here — see the type's documentation.
    pub fn scope(&self) -> Option<(&str, &str)> {
        self.0.split_once(':')
    }
}

impl std::fmt::Display for CapabilityRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CapabilityRequest {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::new(&raw).map_err(serde::de::Error::custom)
    }
}

/// What a character wants to be able to do.
///
/// A set, so authoring the same request twice is the same request — and so the order in the
/// file never becomes meaningful.
pub type RequestedCapabilities = BTreeSet<CapabilityRequest>;

/// Who does this character's thinking — and, crucially, **who owns the loop** (ADR-0027).
///
/// A *model* thinks. Epoch composes its context, judges every tool call, runs the capability and
/// records the evidence. An *agent* thinks **and works**: it plans, reads, edits and runs things
/// on its own, and some minutes later it is done. Dressing the second up as the first would be a
/// lie the type system would help us tell — every caller would believe Epoch was still driving.
///
/// So the distinction lives in the domain rather than in a flag, and it is visible in the
/// interface rather than inferred.
///
/// ## The discriminant is which name is present
///
/// `provider` for a model, `agent` for an agent, and exactly one of them. That keeps the authored
/// file flat and readable — no `kind = "model"` line for the overwhelmingly common case — and it
/// means every character file written before agents existed already says what it means. There is
/// no migration, because nothing about the old shape was wrong.
///
/// Both names, or neither, is refused rather than guessed at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Brain {
    /// It thinks. Epoch owns the loop, the tools, the trust and the record.
    Model {
        /// Canonical backend id — `"ollama"`, `"laptop"`, `"anthropic"`, … Which one you mean,
        /// never which kind it is (see `epoch-engine::backends`).
        provider: String,
        /// The model, exactly as that backend names it. Never normalised: it is their word.
        model: String,
    },
    /// It thinks *and works*. The agent owns the loop; Epoch owns the gate and the record.
    Agent {
        /// Canonical agent id — `"claude_code"`, `"codex"`, …
        agent: String,
        /// Which model the agent should use, in the agent's own vocabulary.
        model: String,
    },
}

impl Brain {
    /// The model name, in whichever vocabulary applies. Both kinds have one.
    pub fn model(&self) -> &str {
        match self {
            Brain::Model { model, .. } | Brain::Agent { model, .. } => model,
        }
    }

    /// The backend that would do the thinking, or `None` when an agent would.
    ///
    /// Deliberately an `Option` rather than a string that happens to be an agent id: a call site
    /// reaching for a `Provider` must be made to notice that there might not be one. That is the
    /// whole reason this is an enum and not a flag.
    pub fn provider(&self) -> Option<&str> {
        match self {
            Brain::Model { provider, .. } => Some(provider),
            Brain::Agent { .. } => None,
        }
    }

    /// The agent that would do the work, or `None` when a model would.
    pub fn agent(&self) -> Option<&str> {
        match self {
            Brain::Agent { agent, .. } => Some(agent),
            Brain::Model { .. } => None,
        }
    }

    /// Which name identifies the thing that thinks, whichever kind it is. For display only.
    pub fn who(&self) -> &str {
        match self {
            Brain::Model { provider, .. } => provider,
            Brain::Agent { agent, .. } => agent,
        }
    }
}

/// Who does this character's thinking, how, and tuned how.
///
/// **The name and the model together, on purpose.** A model name is only meaningful to whoever
/// offers it — `qwen3:14b` means nothing to Anthropic and `claude-opus-5` means nothing to a
/// local runtime. Storing the name alone would leave the Engine guessing whose dialect it is.
///
/// The model is infrastructure; the Character is the product (`CHARACTER_BIBLE.md`). Mage is Mage
/// whichever of these is behind her, and this is precisely what may be swapped without her
/// becoming somebody else.
#[derive(Debug, Clone, PartialEq)]
pub struct Mind {
    /// What thinks, and who owns the loop.
    pub brain: Brain,
    /// Canonical, portable, and the same idea on every backend.
    pub parameters: Parameters,
    /// Provider-native, kept when the provider changes rather than deleted — moving a character
    /// back restores their tuning, and a dormant namespace is shown as dormant rather than
    /// silently applied.
    pub tuning: Tuning,
}

/// The authored shape, and the only thing that touches a file.
///
/// Written out rather than derived on `Mind` because the enum is flat in the file and nested in
/// the type. `#[serde(flatten)]` expresses exactly that and cannot be used here: it makes TOML
/// serialise the whole thing inline, and these are tables in a file people read.
///
/// Field order is load-bearing: TOML requires every table after every value.
#[derive(Serialize, Deserialize)]
struct Authored {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    agent: Option<String>,
    /// **Absent when nobody chose one**, which only an agent can be: a provider without a model
    /// cannot be asked anything, but an agent's own default is a real and common answer.
    ///
    /// Written as an absent key rather than `model = ""`. The vault is meant to be opened by
    /// hand, and an empty string there reads as a mistake somebody made rather than as a choice
    /// nobody needed to make.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    model: String,
    #[serde(default, skip_serializing_if = "Parameters::is_empty")]
    parameters: Parameters,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    tuning: Tuning,
}

impl Serialize for Mind {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        Authored {
            provider: self.brain.provider().map(str::to_owned),
            agent: self.brain.agent().map(str::to_owned),
            model: self.brain.model().to_owned(),
            parameters: self.parameters.clone(),
            tuning: self.tuning.clone(),
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for Mind {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let authored = Authored::deserialize(d)?;
        let brain = match (authored.provider, authored.agent) {
            (Some(provider), None) => Brain::Model {
                provider,
                model: authored.model,
            },
            (None, Some(agent)) => Brain::Agent {
                agent,
                model: authored.model,
            },
            // Either of these, guessed at, would pick somebody's thinking for them.
            (Some(_), Some(_)) => {
                return Err(serde::de::Error::custom(
                    "a mind names either a provider or an agent, and this one names both",
                ))
            }
            (None, None) => {
                return Err(serde::de::Error::custom(
                    "a mind must name a provider or an agent",
                ))
            }
        };
        Ok(Self {
            brain,
            parameters: authored.parameters,
            tuning: authored.tuning,
        })
    }
}

impl Mind {
    /// The smallest honest Mind: a model to think with, nothing tuned.
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self::of(Brain::Model {
            provider: provider.into(),
            model: model.into(),
        })
    }

    /// The smallest honest Mind around any brain.
    pub fn of(brain: Brain) -> Self {
        Self {
            brain,
            parameters: Parameters::default(),
            tuning: Tuning::new(),
        }
    }

    /// The model, in whichever vocabulary applies.
    pub fn model(&self) -> &str {
        self.brain.model()
    }

    /// The backend that thinks, or `None` when an agent works instead.
    pub fn provider(&self) -> Option<&str> {
        self.brain.provider()
    }

    /// How a surface refers to one choice. Not parsed back — an id pair, flattened for display
    /// and for a `<select>` value, and nothing downstream splits it again.
    pub fn label(&self) -> String {
        format!("{} · {}", self.brain.who(), self.brain.model())
    }

    /// The provider-native settings that apply *right now* — those of the assigned provider.
    ///
    /// Every other namespace is dormant, and an agent has none at all: it manages its own
    /// prompts, so a namespace of `num_ctx` would be settings nothing reads.
    pub fn active_tuning(&self) -> Option<&BTreeMap<String, TuningValue>> {
        self.tuning.get(self.brain.provider()?)
    }

    /// Namespaces held for something other than the assigned provider, in order.
    ///
    /// Exists so a surface can *show* that they are being kept — the alternative is a file
    /// containing settings the user cannot see and cannot explain.
    pub fn dormant_tuning(&self) -> impl Iterator<Item = &String> {
        let active = self.brain.provider();
        self.tuning
            .keys()
            .filter(move |p| Some(p.as_str()) != active)
    }

    /// Every reason this Mind is unusable, or none.
    pub fn problems(&self) -> Vec<String> {
        let mut found = Vec::new();
        if self.brain.who().trim().is_empty() {
            found.push(match self.brain {
                Brain::Model { .. } => "a mind must name a provider".into(),
                Brain::Agent { .. } => "a mind must name an agent".into(),
            });
        }
        if self.brain.model().trim().is_empty() {
            found.push("a mind must name a model".into());
        }
        found.extend(self.parameters.problems());
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn whole(name: &str, min: i64, max: i64) -> Control {
        Control {
            name: name.into(),
            label: name.into(),
            help: String::new(),
            kind: ControlKind::Whole { min, max },
            default: None,
            measured: false,
        }
    }

    #[test]
    fn a_control_refuses_a_value_outside_the_bounds_it_declared() {
        // Checked here rather than trusted from a surface. A surface's validation is a
        // courtesy; this is the control.
        let c = whole("num_ctx", 512, 32_768);
        assert!(c.accepts(&TuningValue::Int(4096)));
        assert!(!c.accepts(&TuningValue::Int(1_000_000)));
        assert!(!c.accepts(&TuningValue::Int(0)));
        // And the wrong shape entirely is not a near miss, it is a different thing.
        assert!(!c.accepts(&TuningValue::Text("4096".into())));
        assert!(!c.accepts(&TuningValue::Bool(true)));
    }

    #[test]
    fn a_whole_number_is_an_acceptable_ratio() {
        // A model or a surface that sends `1` where `1.0` was expected is not making a mistake
        // worth refusing — the value is unambiguous.
        let c = Control {
            kind: ControlKind::Ratio { min: 0.0, max: 2.0 },
            ..whole("repeat_penalty", 0, 2)
        };
        assert!(c.accepts(&TuningValue::Int(1)));
        assert!(c.accepts(&TuningValue::Float(1.15)));
        assert!(!c.accepts(&TuningValue::Int(9)));
    }

    #[test]
    fn a_choice_is_only_one_of_the_things_offered() {
        let c = Control {
            kind: ControlKind::Choice {
                options: vec!["f16".into(), "q8_0".into()],
            },
            ..whole("cache_type", 0, 0)
        };
        assert!(c.accepts(&TuningValue::Text("q8_0".into())));
        assert!(!c.accepts(&TuningValue::Text("q4_0".into())));
    }

    #[test]
    fn a_control_survives_the_round_trip_a_surface_puts_it_through() {
        // It crosses to the interface as JSON and is rendered generically (ADR-0003). A shape
        // that did not round-trip would be a surface guessing.
        let c = Control {
            default: Some(TuningValue::Int(4096)),
            measured: true,
            ..whole("num_ctx", 512, 32_768)
        };
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(serde_json::from_str::<Control>(&json).unwrap(), c);
        // `measured` is the difference between a fact and a guess, so it has to survive.
        assert!(json.contains("\"measured\":true"));
    }

    #[test]
    fn a_reasoning_level_arriving_from_a_surface_is_refused_if_invented() {
        assert_eq!(Reasoning::from_id("high"), Some(Reasoning::High));
        assert_eq!(Reasoning::from_id("  MAX "), Some(Reasoning::Max));
        assert_eq!(Reasoning::from_id("ultra"), None);
        assert_eq!(Reasoning::from_id(""), None);
    }

    #[test]
    fn nothing_is_defaulted_so_unset_stays_distinguishable_from_chosen() {
        let p = Parameters::default();
        assert!(p.is_empty());
        assert_eq!(p.temperature, None);
        // A value the user actually chose is not empty, even when it equals a common default.
        let chosen = Parameters {
            temperature: Some(0.7),
            ..Parameters::default()
        };
        assert!(!chosen.is_empty());
    }

    #[test]
    fn an_out_of_range_parameter_is_refused_rather_than_clamped() {
        let p = Parameters {
            temperature: Some(7.0),
            top_p: Some(-0.1),
            context_tokens: Some(16),
            context_policy: None,
            reasoning: None,
        };
        let problems = p.problems();
        assert_eq!(problems.len(), 3, "{problems:?}");
        // Clamping would leave the file saying one thing and the model doing another.
        assert!(problems[0].contains("temperature"));
        assert!(problems[2].contains("context_tokens"));
    }

    #[test]
    fn the_edges_of_each_range_are_allowed() {
        let p = Parameters {
            temperature: Some(0.0),
            top_p: Some(1.0),
            context_tokens: Some(256),
            context_policy: Some(ContextPolicy::Long),
            reasoning: Some(Reasoning::Off),
        };
        assert!(p.problems().is_empty());
    }

    #[test]
    fn the_floor_wins_over_the_policy() {
        /*
            The owner's correction, and it is the rule: a `Compact` preference of 8K on a character
            whose required blocks cost 12K produces a **12K** character, not an 8K one. The policy
            divides `window - required`; required is spent before it is asked, because ADR-0012
            forbids dropping it.
        */
        let it = effective_use(32_768, 12_000, ContextPolicy::Compact, None).expect("it fits");
        assert!(it >= 12_000, "the floor: {it}");
        assert_eq!(it, 12_000 + 5_192, "and a quarter of what was optional");
    }

    #[test]
    fn a_character_that_cannot_run_says_so_rather_than_returning_a_small_number() {
        // Required larger than the window is a configuration error to be found when a profile is
        // applied, not a turn that fails.
        assert_eq!(
            effective_use(32_768, 38_000, ContextPolicy::Long, None),
            None
        );
    }

    #[test]
    fn the_policy_divides_what_is_optional_and_never_the_whole_window() {
        // `Long` on a 64K window with a 4K floor is 4K plus 90% of 60K, not 90% of 64K.
        let long = effective_use(65_536, 4_096, ContextPolicy::Long, None).expect("fits");
        assert_eq!(long, 4_096 + ((65_536 - 4_096) as f64 * 0.90) as u32);
        assert!(long < 65_536, "and it never reaches the ceiling");
    }

    #[test]
    fn adaptive_takes_the_whole_window_as_its_ceiling_rather_than_a_fraction() {
        /*
            It is not a fraction. It is *the Composer decides from the turn*, and returning a number
            from the policy would be inventing the answer the policy exists to work out.
        */
        assert_eq!(ContextPolicy::Adaptive.share_of_optional(), None);
        assert_eq!(
            effective_use(65_536, 4_096, ContextPolicy::Adaptive, None),
            Some(65_536),
        );
    }

    #[test]
    fn a_character_may_lower_its_own_ceiling_and_never_raise_it() {
        /*
            The surviving half of `context_tokens`: asking to hold *less* is a request any backend
            can satisfy. Asking for more than the window is not, and it is ignored rather than
            honoured — the window belongs to MODELS.
        */
        assert_eq!(
            effective_use(65_536, 4_096, ContextPolicy::Long, Some(16_384)),
            Some(16_384),
        );
        let higher = effective_use(65_536, 4_096, ContextPolicy::Long, Some(262_144));
        assert!(higher.expect("fits") <= 65_536);
        // And never below the floor, whatever was asked for.
        assert_eq!(
            effective_use(65_536, 12_000, ContextPolicy::Compact, Some(500)),
            Some(12_000),
        );
    }

    #[test]
    fn the_default_policy_is_adaptive_and_none_of_them_is_called_auto() {
        // A character that says nothing gets the one that decides per turn: any fraction would be
        // a preference nobody expressed. And MODELS keeps `AUTO` for choosing a physical profile —
        // two layers, two words.
        assert_eq!(ContextPolicy::default(), ContextPolicy::Adaptive);
        assert_eq!(Parameters::default().context_policy, None);
        for one in [
            ContextPolicy::Compact,
            ContextPolicy::Adaptive,
            ContextPolicy::Long,
            ContextPolicy::Custom,
        ] {
            assert!(!one.name().eq_ignore_ascii_case("auto"), "{}", one.name());
            assert!(!one.about().is_empty());
        }
    }

    #[test]
    fn a_policy_is_written_and_read_back_by_one_table() {
        /*
            The parse side and the print side are one fact, and when a fact is written down twice
            the half nobody is watching is the half where the two spellings differ. It has cost
            this repository a dark AUDIO tab already — `stableaudio` against `Stable Audio`, which
            agreed for every family whose name has no space in it.

            These four agree today. `from_id` is derived from `name` so they cannot stop.
        */
        for one in ContextPolicy::ALL {
            assert_eq!(ContextPolicy::from_id(&one.id()), Some(one), "{}", one.id());
        }
        assert_eq!(ContextPolicy::ALL.len(), 4);
        // A word this build does not know is refused, never defaulted — a file saying `lnog`
        // would otherwise load as Adaptive and behave as something nobody asked for.
        assert_eq!(ContextPolicy::from_id("lnog"), None);
        assert_eq!(ContextPolicy::from_id(""), None);
        assert_eq!(
            ContextPolicy::from_id("Compact"),
            None,
            "the id is lowercase"
        );
        // Whitespace from a hand-edited file is not a different policy.
        assert_eq!(ContextPolicy::from_id("  long "), Some(ContextPolicy::Long));
    }

    #[test]
    fn only_the_assigned_providers_tuning_is_active() {
        let mut mind = Mind::new("ollama", "qwen3:14b");
        mind.tuning
            .entry("ollama".into())
            .or_default()
            .insert("num_ctx".into(), TuningValue::Int(32768));
        mind.tuning
            .entry("anthropic".into())
            .or_default()
            .insert("something_else".into(), TuningValue::Bool(true));

        assert_eq!(
            mind.active_tuning().and_then(|t| t.get("num_ctx")),
            Some(&TuningValue::Int(32768))
        );
        // Kept, not applied — and visible as kept.
        assert_eq!(mind.dormant_tuning().collect::<Vec<_>>(), vec!["anthropic"]);
    }

    #[test]
    fn switching_provider_keeps_the_old_namespace() {
        let mut mind = Mind::new("ollama", "qwen3:14b");
        mind.tuning
            .entry("ollama".into())
            .or_default()
            .insert("num_ctx".into(), TuningValue::Int(8192));

        mind.brain = Brain::Model {
            provider: "anthropic".into(),
            model: "claude-opus-5".into(),
        };

        assert!(
            mind.active_tuning().is_none(),
            "nothing authored for the new provider yet"
        );
        assert_eq!(mind.dormant_tuning().collect::<Vec<_>>(), vec!["ollama"]);
        // Moving back restores it, which is the whole reason it is kept.
        mind.brain = Brain::Model {
            provider: "ollama".into(),
            model: "qwen3:14b".into(),
        };
        assert!(mind.active_tuning().is_some());
    }

    #[test]
    fn a_file_written_before_agents_existed_still_says_what_it_means() {
        // The discriminant is which name is present, so there is no migration: every character
        // file already written parses as a model, unchanged.
        let mind: Mind =
            serde_json::from_str(r#"{"provider":"ollama","model":"qwen3:14b"}"#).unwrap();
        assert_eq!(
            mind.brain,
            Brain::Model {
                provider: "ollama".into(),
                model: "qwen3:14b".into()
            }
        );
        // And it is written back the way it was read, rather than gaining a `kind` line nobody
        // typed. (That the TOML shape stays value-before-table is asserted where a whole
        // character file round-trips, in `epoch-engine::definition`.)
        assert_eq!(
            serde_json::to_string(&mind).unwrap(),
            r#"{"provider":"ollama","model":"qwen3:14b"}"#
        );
    }

    #[test]
    fn an_agent_is_a_different_kind_of_brain_and_says_so() {
        let mind: Mind = serde_json::from_str(r#"{"agent":"claude_code","model":"opus"}"#).unwrap();
        assert_eq!(mind.brain.agent(), Some("claude_code"));
        // A caller reaching for a Provider is made to notice there is not one — which is the
        // whole reason this is an enum rather than a flag (ADR-0027).
        assert_eq!(mind.provider(), None);
        // An agent manages its own prompts, so provider-native tuning has no namespace to be in.
        assert!(mind.active_tuning().is_none());
    }

    #[test]
    fn a_mind_that_names_both_or_neither_is_refused_rather_than_guessed_at() {
        // Either guess would pick somebody's thinking for them.
        assert!(serde_json::from_str::<Mind>(
            r#"{"provider":"ollama","agent":"claude_code","model":"m"}"#
        )
        .is_err());
        assert!(serde_json::from_str::<Mind>(r#"{"model":"m"}"#).is_err());
    }

    #[test]
    fn a_requested_capability_is_intent_and_is_validated_for_shape() {
        assert_eq!(
            CapabilityRequest::new("web_search").unwrap().as_str(),
            "web_search"
        );
        assert!(CapabilityRequest::new("").is_err());
        assert!(CapabilityRequest::new("Web Search").is_err());
        assert!(CapabilityRequest::new("web-search").is_err());
    }

    #[test]
    fn a_request_may_name_a_whole_source_and_the_kernel_does_not_interpret_it() {
        let one = CapabilityRequest::new("read_file").unwrap();
        assert_eq!(one.scope(), None, "a plain capability names no source");

        let source = CapabilityRequest::new("mcp:playwright").unwrap();
        assert_eq!(source.scope(), Some(("mcp", "playwright")));
        assert_eq!(
            source.as_str(),
            "mcp:playwright",
            "what was authored is what round-trips; the Kernel splits it, never rewrites it"
        );

        // Half a scope resolves to nothing, which would look like a capability that never
        // arrives rather than a file that is wrong.
        assert!(CapabilityRequest::new("mcp:").is_err());
        assert!(CapabilityRequest::new(":playwright").is_err());
        assert!(CapabilityRequest::new("mcp:a:b").is_err());
        assert!(CapabilityRequest::new("mcp:Playwright").is_err());
    }

    #[test]
    fn requesting_the_same_capability_twice_is_one_request() {
        let mut wanted = RequestedCapabilities::new();
        wanted.insert(CapabilityRequest::new("vision").unwrap());
        wanted.insert(CapabilityRequest::new("vision").unwrap());
        assert_eq!(wanted.len(), 1);
    }

    #[test]
    fn a_mind_with_no_model_is_refused() {
        let mind = Mind::new("ollama", "  ");
        assert_eq!(
            mind.problems(),
            vec!["a mind must name a model".to_string()]
        );
    }
}
