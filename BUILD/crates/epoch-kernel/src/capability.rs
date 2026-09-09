//! Executable Capabilities — what the World is able to do (ADR-0008, ADR-0009).
//!
//! A capability **never knows who invoked it**. A character, a Quest, a scheduler and the CLI
//! all reach it through the identical contract, which is what lets one gate secure everything
//! rather than one gate per caller (ADR-0008).
//!
//! ## Two halves, and only one of them is static
//!
//! A [`Descriptor`] is what a capability *is*: its name, what kind of damage it can do, and
//! whether that damage can be taken back. It never changes.
//!
//! An [`Explanation`] is what one *particular* execution would do — "read `src/main.rs`",
//! "delete 14 files" — and it can only exist once the arguments are known. ADR-0009 requires
//! the user be told what will happen before it happens, and "what will happen" is a fact about
//! the call, not about the capability.
//!
//! ## Risk is derived, never merely claimed
//!
//! ADR-0009 listed Risk Level as something a capability declares, and flagged the assumption:
//! *"capability authors can classify side effects/risk accurately."* They cannot, reliably —
//! and a declared risk that disagrees with the declared effects is worse than no risk at all,
//! because the user is reading the number rather than the effects.
//!
//! So risk is **computed** from what the capability admits it does and whether it can be
//! undone. An author may [`Descriptor::at_least`] to raise it — some things are worse than they
//! look — and there is deliberately no way to lower it. Declaring `Deletes` and `Low` is not
//! expressible.
//!
//! ## No vendor's schema in here
//!
//! Parameters are described in canonical terms and rendered into whatever a provider's
//! tool-calling API wants, in that provider. The Kernel does not know what JSON Schema is, for
//! the same reason it does not know what `num_ctx` is (ADR-0026).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Stable identity of a capability.
///
/// Constrained on construction so nothing downstream has to re-check it, and so a name arriving
/// from a model — which is where these arrive from — cannot be a path, a shell fragment or a
/// namespace nobody registered.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct CapabilityId(String);

impl CapabilityId {
    pub fn new(raw: &str) -> Result<Self, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("a capability id cannot be empty".into());
        }
        if let Some(bad) = trimmed
            .chars()
            .find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_'))
        {
            return Err(format!(
                "capability id '{trimmed}' contains '{bad}'; use lowercase letters, digits or '_'"
            ));
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CapabilityId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::new(&raw).map_err(serde::de::Error::custom)
    }
}

/// One kind of mark a capability leaves on the world outside Epoch.
///
/// A set, not a choice: running a shell command executes code, and that code may well write
/// files and open sockets. A capability that admits only the most flattering of its effects is
/// the failure this type exists to make awkward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effect {
    /// Observes. Changes nothing anywhere.
    Reads,
    /// Creates or modifies something that persists.
    Writes,
    /// Removes something. Separate from `Writes` because the two fail differently: a bad write
    /// leaves a wrong file, a bad delete leaves no file.
    Deletes,
    /// Runs code Epoch did not write and cannot inspect. Implies anything that code can do.
    Executes,
    /// Leaves the machine. The user's data goes somewhere they did not type, or somebody
    /// else's data arrives — and arriving data is never trustworthy.
    Network,
}

impl Effect {
    pub const ALL: [Effect; 5] = [
        Effect::Reads,
        Effect::Writes,
        Effect::Deletes,
        Effect::Executes,
        Effect::Network,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Effect::Reads => "reads",
            Effect::Writes => "writes",
            Effect::Deletes => "deletes",
            Effect::Executes => "executes",
            Effect::Network => "network",
        }
    }

    /// Plain words for the person being asked to approve this.
    pub fn describe(self) -> &'static str {
        match self {
            Effect::Reads => "reads without changing anything",
            Effect::Writes => "creates or changes files",
            Effect::Deletes => "removes things",
            Effect::Executes => "runs code on this machine",
            Effect::Network => "sends or receives over the network",
        }
    }

    /// Whether this effect alone leaves the world outside Epoch unchanged.
    pub fn is_observation(self) -> bool {
        matches!(self, Effect::Reads)
    }
}

impl std::fmt::Display for Effect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether what a capability did can be taken back.
///
/// The single most useful thing to know before approving something, and the thing a permission
/// checkbox never tells you: "run a command" is a different decision when the answer is *this
/// cannot be undone*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "how")]
pub enum Reversal {
    /// Nothing happened that would need undoing. Only honest alongside `Reads`.
    NothingToUndo,
    /// A compensating action exists, named here so the explanation can say what it is.
    Undoable(String),
    /// Cannot be undone by Epoch. A `git push`, a deleted file, a sent request.
    Permanent,
}

impl Reversal {
    pub fn describe(&self) -> String {
        match self {
            Reversal::NothingToUndo => "nothing to undo".into(),
            Reversal::Undoable(how) => format!("can be undone: {how}"),
            Reversal::Permanent => "cannot be undone".into(),
        }
    }

    pub fn is_permanent(&self) -> bool {
        matches!(self, Reversal::Permanent)
    }
}

/// How much this could cost if it goes wrong. **Derived** — see the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    /// Observation only. Explorer mode allows exactly this and nothing else (ADR-0009).
    None,
    /// Changes something, and the change can be taken back.
    Low,
    /// Changes something permanently, or leaves the machine.
    Medium,
    /// Deletes or executes, permanently. The tier that always asks, whatever the mode.
    High,
}

impl Risk {
    pub fn as_str(self) -> &'static str {
        match self {
            Risk::None => "none",
            Risk::Low => "low",
            Risk::Medium => "medium",
            Risk::High => "high",
        }
    }
}

impl std::fmt::Display for Risk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What kind of value a parameter takes.
///
/// Four scalars, deliberately. A capability wanting a nested object has an argument shape a
/// model will get wrong and a user cannot read in an explanation; it should take a path or an
/// id instead. This grows when something real needs it and not before.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValueKind {
    Text,
    Integer,
    Number,
    Boolean,
}

impl ValueKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ValueKind::Text => "text",
            ValueKind::Integer => "integer",
            ValueKind::Number => "number",
            ValueKind::Boolean => "boolean",
        }
    }
}

/// One argument a capability accepts, in canonical terms.
///
/// Rendered into a provider's own tool-calling schema by that provider — never here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    /// What it is for, in words a model reads and a person could too.
    pub description: String,
    pub kind: ValueKind,
    /// A capability may not be invoked without this.
    pub required: bool,
}

impl Parameter {
    pub fn required(name: &str, kind: ValueKind, description: &str) -> Self {
        Self {
            name: name.to_owned(),
            description: description.to_owned(),
            kind,
            required: true,
        }
    }

    pub fn optional(name: &str, kind: ValueKind, description: &str) -> Self {
        Self {
            name: name.to_owned(),
            description: description.to_owned(),
            kind,
            required: false,
        }
    }
}

/// One argument's value, as it arrives.
///
/// Structurally the same as [`crate::TuningValue`], and deliberately not the same type. They
/// answer different questions — a provider's private vocabulary versus a capability's public
/// arguments — and merging them would couple two subsystems through a shape they happen to
/// share today. Two is a coincidence; a third is when we extract one (Earn Complexity).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Boolean(bool),
    Integer(i64),
    Number(f64),
    Text(String),
}

impl Value {
    pub fn kind(&self) -> ValueKind {
        match self {
            Value::Boolean(_) => ValueKind::Boolean,
            Value::Integer(_) => ValueKind::Integer,
            Value::Number(_) => ValueKind::Number,
            Value::Text(_) => ValueKind::Text,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Boolean(v) => write!(f, "{v}"),
            Value::Integer(v) => write!(f, "{v}"),
            Value::Number(v) => write!(f, "{v}"),
            Value::Text(v) => f.write_str(v),
        }
    }
}

/// The arguments of one call.
///
/// These come from a **model**, which is to say from something that guesses. Nothing here is
/// assumed to be present, of the right type, or sane — [`Arguments::check`] is what a
/// capability runs before it believes any of it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Arguments(std::collections::BTreeMap<String, Value>);

impl Arguments {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, name: &str, value: Value) -> Self {
        self.0.insert(name.to_owned(), value);
        self
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.0.get(name)
    }

    /// A required text argument, or the reason it is unusable.
    pub fn text(&self, name: &str) -> Result<&str, String> {
        match self.0.get(name) {
            Some(Value::Text(value)) => Ok(value),
            Some(other) => Err(format!(
                "'{name}' should be text, not {}",
                other.kind().as_str()
            )),
            None => Err(format!("'{name}' is required")),
        }
    }

    /// An **optional** text argument, reading blank as absent.
    ///
    /// A model that means "no value" often has no way to say so. The tool schema names the
    /// field, so it fills it in with `""` — and a capability that reads that as a value turns
    /// an omission into a refusal. Measured: `list_files` called with `path: ""` answered
    /// *"a path is required"* to a model that had correctly asked for the project root, and
    /// the turn was spent on the refusal. **Blank is how a model omits.**
    ///
    /// The rule already existed — `draw_image` spelled it out by hand for `style` and
    /// `from_image` — it simply was not anywhere the next capability could find it.
    ///
    /// **Optional parameters only.** A *required* argument arriving blank is a real problem
    /// and the capability that wants it still says so; this must never be used to invent a
    /// value for something the caller was obliged to supply.
    pub fn optional_text(&self, name: &str) -> Option<&str> {
        match self.0.get(name) {
            Some(Value::Text(value)) if !value.trim().is_empty() => Some(value.trim()),
            _ => None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.0.iter()
    }

    /// Everything wrong with these arguments against a capability's parameters.
    ///
    /// Unknown arguments are refused rather than ignored. A model that invented `recursive:
    /// true` believes it asked for recursion; silently dropping it means the user approved one
    /// thing and something else ran.
    pub fn check(&self, parameters: &[Parameter]) -> Vec<String> {
        let mut found = Vec::new();
        for parameter in parameters {
            match self.0.get(&parameter.name) {
                Some(value) => {
                    // An integer offered where a number is wanted is the one widening a model
                    // does constantly and which is never a mistake.
                    let ok = value.kind() == parameter.kind
                        || (parameter.kind == ValueKind::Number
                            && value.kind() == ValueKind::Integer);
                    if !ok {
                        found.push(format!(
                            "'{}' should be {}, not {}",
                            parameter.name,
                            parameter.kind.as_str(),
                            value.kind().as_str()
                        ));
                    }
                }
                None if parameter.required => {
                    found.push(format!("'{}' is required", parameter.name));
                }
                None => {}
            }
        }
        for name in self.0.keys() {
            if !parameters.iter().any(|p| &p.name == name) {
                found.push(format!("'{name}' is not an argument this takes"));
            }
        }
        found
    }
}

/// What a capability *is*. Static, and the same for every caller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Descriptor {
    pub id: CapabilityId,
    /// One line. Shown to the user, and given to the model as the tool's description.
    pub summary: String,
    /// Every mark it can leave. Understated effects are the failure mode this guards against.
    pub effects: BTreeSet<Effect>,
    pub reversal: Reversal,
    pub parameters: Vec<Parameter>,
    /// A floor the author may set when something is worse than its effects suggest.
    ///
    /// Raises only. There is no field for lowering it, on purpose: "trust me, deleting is fine
    /// here" is exactly the claim that must not be expressible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    floor: Option<Risk>,
}

impl Descriptor {
    /// A capability that only looks. The one shape Explorer mode allows.
    pub fn observing(id: CapabilityId, summary: impl Into<String>) -> Self {
        Self {
            id,
            summary: summary.into(),
            effects: BTreeSet::from([Effect::Reads]),
            reversal: Reversal::NothingToUndo,
            parameters: Vec::new(),
            floor: None,
        }
    }

    /// A capability that changes something.
    pub fn acting(
        id: CapabilityId,
        summary: impl Into<String>,
        effects: impl IntoIterator<Item = Effect>,
        reversal: Reversal,
    ) -> Self {
        Self {
            id,
            summary: summary.into(),
            effects: effects.into_iter().collect(),
            reversal,
            parameters: Vec::new(),
            floor: None,
        }
    }

    pub fn taking(mut self, parameters: impl IntoIterator<Item = Parameter>) -> Self {
        self.parameters = parameters.into_iter().collect();
        self
    }

    /// Raise the risk floor. Lowering is not offered.
    pub fn at_least(mut self, risk: Risk) -> Self {
        self.floor = Some(match self.floor {
            Some(existing) => existing.max(risk),
            None => risk,
        });
        self
    }

    /// Whether this leaves the world outside Epoch exactly as it found it.
    ///
    /// Network counts as changing it: a request is a thing that happened to somebody else, and
    /// what comes back is untrusted input. "Read-only" must not quietly include "and talks to
    /// the internet".
    pub fn is_observation(&self) -> bool {
        !self.effects.is_empty() && self.effects.iter().all(|e| e.is_observation())
    }

    /// What this could cost, computed from what it admits doing.
    ///
    /// Never below the floor, and never inconsistent with the effects — which is the whole
    /// reason it is not a field.
    pub fn risk(&self) -> Risk {
        let derived = if self.is_observation() {
            Risk::None
        } else if self.effects.contains(&Effect::Deletes)
            || self.effects.contains(&Effect::Executes)
        {
            // Running arbitrary code is the ceiling regardless of what it claims to do with it,
            // because nothing here can see inside it.
            if self.reversal.is_permanent() {
                Risk::High
            } else {
                Risk::Medium
            }
        } else if self.reversal.is_permanent() || self.effects.contains(&Effect::Network) {
            Risk::Medium
        } else {
            Risk::Low
        };

        match self.floor {
            Some(floor) => derived.max(floor),
            None => derived,
        }
    }

    /// Everything wrong with this declaration, or none.
    ///
    /// A capability whose descriptor contradicts itself would produce an explanation the user
    /// cannot rely on, so it is refused at registration rather than at the moment it runs.
    pub fn problems(&self) -> Vec<String> {
        let mut found = Vec::new();
        if self.summary.trim().is_empty() {
            found.push(format!(
                "capability '{}' does not say what it does",
                self.id
            ));
        }
        if self.effects.is_empty() {
            found.push(format!(
                "capability '{}' declares no effects; a capability that does nothing at all \
                 cannot be explained",
                self.id
            ));
        }
        // The contradiction that matters: something that changed the world claiming there is
        // nothing to undo. Every other combination is at worst pessimistic.
        if self.reversal == Reversal::NothingToUndo && !self.is_observation() {
            found.push(format!(
                "capability '{}' changes something but claims there is nothing to undo",
                self.id
            ));
        }
        for parameter in &self.parameters {
            if parameter.name.trim().is_empty() {
                found.push(format!("capability '{}' has an unnamed parameter", self.id));
            }
            if parameter.description.trim().is_empty() {
                found.push(format!(
                    "capability '{}' does not say what '{}' is for",
                    self.id, parameter.name
                ));
            }
        }
        found
    }
}

/// What one particular execution would do, said before it happens (ADR-0009).
///
/// This is the sentence a character says out loud — *"I'll run `cargo test`; read-only,
/// nothing to undo"* — and the thing the user approves. Trust stops being a modal dialog and
/// becomes somebody asking you something.
///
/// It carries the risk and reversal it was built with rather than looking them up later, so
/// what was approved and what runs cannot drift apart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Explanation {
    pub capability: CapabilityId,
    /// What will happen, concretely, with the arguments filled in. Not the capability's
    /// summary — "read `src/main.rs`", not "reads a file".
    pub what: String,
    /// Why it is being done now, when the caller knows. `None` rather than an invented reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    /// What is expected to come back, when that can be said without doing it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expect: Option<String>,
    /// The change itself, when a capability can show it without making it.
    ///
    /// ADR-0009 lists **Preview Support** among what a capability declares, and this is it. The
    /// difference between "replace `src/main.rs` — 40 lines become 41" and the actual lines
    /// being replaced is the difference between a description and evidence: only one of them
    /// lets somebody notice that the wrong thing is about to happen.
    ///
    /// `None` when the capability genuinely cannot say — never a summary dressed as a preview.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    pub effects: BTreeSet<Effect>,
    pub reversal: Reversal,
    pub risk: Risk,
}

impl Explanation {
    /// Build the explanation for a call, taking the facts from the descriptor so they cannot
    /// be overstated by a caller in a hurry.
    pub fn of(descriptor: &Descriptor, what: impl Into<String>) -> Self {
        Self {
            capability: descriptor.id.clone(),
            what: what.into(),
            why: None,
            expect: None,
            preview: None,
            effects: descriptor.effects.clone(),
            reversal: descriptor.reversal.clone(),
            risk: descriptor.risk(),
        }
    }

    pub fn because(mut self, why: impl Into<String>) -> Self {
        self.why = Some(why.into());
        self
    }

    pub fn expecting(mut self, expect: impl Into<String>) -> Self {
        self.expect = Some(expect.into());
        self
    }

    /// Show the change itself. Only for capabilities that can produce it without making it.
    pub fn showing(mut self, preview: impl Into<String>) -> Self {
        self.preview = Some(preview.into());
        self
    }

    /// One line, for a surface that has room for one line.
    pub fn line(&self) -> String {
        format!("{} — {}", self.what, self.reversal.describe())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(raw: &str) -> CapabilityId {
        CapabilityId::new(raw).unwrap()
    }

    #[test]
    fn an_id_arriving_from_a_model_is_validated_before_anything_uses_it() {
        assert_eq!(id("read_file").as_str(), "read_file");
        assert!(CapabilityId::new("").is_err());
        assert!(CapabilityId::new("Read File").is_err());
        // The one that matters: a name that is a path.
        assert!(CapabilityId::new("../etc/passwd").is_err());
        assert!(CapabilityId::new("rm -rf /").is_err());
        // Namespacing by prefix is allowed and will be needed — it is how MCP names a tool.
        // Refusing it now would be an arbitrary rule to unpick later.
        assert_eq!(
            CapabilityId::new("mcp__thing").unwrap().as_str(),
            "mcp__thing"
        );
    }

    #[test]
    fn observation_is_the_only_shape_that_costs_nothing() {
        let read = Descriptor::observing(id("read_file"), "Read a file from the project.");
        assert!(read.is_observation());
        assert_eq!(read.risk(), Risk::None);
        assert!(read.problems().is_empty());
    }

    #[test]
    fn reaching_the_network_is_not_read_only() {
        // The trap this guards: "it only reads web pages" sounds like observation. It sends
        // the user's query to a stranger and brings back input nobody vetted.
        let fetch = Descriptor::acting(
            id("fetch_url"),
            "Fetch a page and return its text.",
            [Effect::Reads, Effect::Network],
            Reversal::Permanent,
        );
        assert!(!fetch.is_observation());
        assert_eq!(fetch.risk(), Risk::Medium);
    }

    #[test]
    fn risk_follows_the_effects_because_it_is_not_a_field() {
        let write = Descriptor::acting(
            id("write_file"),
            "Write a file.",
            [Effect::Writes],
            Reversal::Undoable("the previous contents are kept".into()),
        );
        assert_eq!(write.risk(), Risk::Low);

        let delete = Descriptor::acting(
            id("delete_file"),
            "Delete a file.",
            [Effect::Deletes],
            Reversal::Permanent,
        );
        assert_eq!(delete.risk(), Risk::High);

        let shell = Descriptor::acting(
            id("run_command"),
            "Run a command.",
            [Effect::Executes, Effect::Writes, Effect::Network],
            Reversal::Permanent,
        );
        assert_eq!(shell.risk(), Risk::High);
    }

    #[test]
    fn a_floor_can_raise_the_risk_and_nothing_can_lower_it() {
        let push = Descriptor::acting(
            id("git_push"),
            "Push commits to the remote.",
            [Effect::Writes, Effect::Network],
            Reversal::Permanent,
        )
        .at_least(Risk::High);
        // Effects alone say Medium; publishing to a place other people read is worse.
        assert_eq!(push.risk(), Risk::High);

        // Raising twice keeps the higher of the two, and there is no API for going down.
        let raised = push.clone().at_least(Risk::Low);
        assert_eq!(raised.risk(), Risk::High);
    }

    #[test]
    fn a_descriptor_that_contradicts_itself_is_refused() {
        let liar = Descriptor::acting(
            id("delete_everything"),
            "Delete things.",
            [Effect::Deletes],
            Reversal::NothingToUndo,
        );
        let problems = liar.problems();
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("nothing to undo"));
    }

    #[test]
    fn a_capability_that_does_nothing_cannot_be_explained() {
        let empty = Descriptor::acting(id("nothing"), "?", [], Reversal::NothingToUndo);
        assert!(empty.problems().iter().any(|p| p.contains("no effects")));
    }

    #[test]
    fn a_parameter_nobody_described_is_refused() {
        // The description is not documentation. It is what the model reads to decide what to
        // put there, and an undescribed parameter produces confident wrong arguments.
        let bad = Descriptor::observing(id("read_file"), "Read a file.")
            .taking([Parameter::required("path", ValueKind::Text, "  ")]);
        assert!(bad
            .problems()
            .iter()
            .any(|p| p.contains("what 'path' is for")));
    }

    #[test]
    fn an_explanation_is_about_the_call_and_takes_its_facts_from_the_capability() {
        let run = Descriptor::acting(
            id("run_command"),
            "Run a command in the project.",
            [Effect::Executes],
            Reversal::Permanent,
        );
        let explained = Explanation::of(&run, "run `cargo test`")
            .because("the plan asks for the tests to pass")
            .expecting("the test output");

        // The concrete call, never the capability's own summary.
        assert_eq!(explained.what, "run `cargo test`");
        // Facts copied from the descriptor: what was approved is what will run.
        assert_eq!(explained.risk, run.risk());
        assert_eq!(explained.reversal, run.reversal);
        assert_eq!(explained.line(), "run `cargo test` — cannot be undone");
    }

    fn reading() -> Descriptor {
        Descriptor::observing(id("read_file"), "Read a file.").taking([
            Parameter::required("path", ValueKind::Text, "Path within the project."),
            Parameter::optional("limit", ValueKind::Integer, "How many lines at most."),
        ])
    }

    #[test]
    fn a_missing_required_argument_is_caught_before_anything_runs() {
        let problems = Arguments::new().check(&reading().parameters);
        assert_eq!(problems, vec!["'path' is required".to_string()]);
    }

    #[test]
    fn an_optional_argument_may_simply_be_absent() {
        let args = Arguments::new().with("path", Value::Text("src/main.rs".into()));
        assert!(args.check(&reading().parameters).is_empty());
        assert_eq!(args.text("path").unwrap(), "src/main.rs");
    }

    #[test]
    fn a_blank_optional_argument_is_how_a_model_omits_one() {
        // Measured: `list_files` with `path: ""` refused with "a path is required", to a model
        // that had asked for the project root in the only way its schema allowed.
        for said in ["", "   ", "\n"] {
            let args = Arguments::new().with("path", Value::Text(said.into()));
            assert_eq!(args.optional_text("path"), None, "for {said:?}");
        }
        let args = Arguments::new().with("path", Value::Text("  src/main.rs ".into()));
        assert_eq!(args.optional_text("path"), Some("src/main.rs"));
        assert_eq!(Arguments::new().optional_text("path"), None);
    }

    #[test]
    fn blank_is_absent_only_for_text_never_for_a_number() {
        // `0` is a value somebody meant. Reading it as an omission is the same mistake in the
        // other direction.
        let args = Arguments::new().with("lines", Value::Integer(0));
        assert_eq!(args.optional_text("lines"), None);
        assert_eq!(args.get("lines"), Some(&Value::Integer(0)));
    }

    #[test]
    fn an_argument_nobody_declared_is_refused_rather_than_ignored() {
        // The failure this prevents: the model believes it asked for recursion, the user
        // approved "read a file", and something else entirely runs.
        let args = Arguments::new()
            .with("path", Value::Text("src".into()))
            .with("recursive", Value::Boolean(true));
        let problems = args.check(&reading().parameters);
        assert_eq!(
            problems,
            vec!["'recursive' is not an argument this takes".to_string()]
        );
    }

    #[test]
    fn a_wrong_type_is_named_rather_than_coerced() {
        let args = Arguments::new().with("path", Value::Integer(7));
        assert_eq!(
            args.check(&reading().parameters),
            vec!["'path' should be text, not integer".to_string()]
        );
        assert!(args.text("path").unwrap_err().contains("should be text"));
    }

    #[test]
    fn an_integer_is_accepted_where_a_number_is_wanted() {
        // The one widening a model does constantly, and which is never a mistake.
        let wants = [Parameter::required(
            "temperature",
            ValueKind::Number,
            "How hot.",
        )];
        let args = Arguments::new().with("temperature", Value::Integer(1));
        assert!(args.check(&wants).is_empty());
    }

    #[test]
    fn nothing_is_invented_when_the_caller_did_not_say() {
        let read = Descriptor::observing(id("read_file"), "Read a file.");
        let explained = Explanation::of(&read, "read src/main.rs");
        assert_eq!(explained.why, None);
        assert_eq!(explained.expect, None);
    }
}
