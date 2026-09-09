//! The capability registry — everything the World is able to do (ADR-0008).
//!
//! ## A capability never learns who called it
//!
//! Not the character, not the Quest, not whether a human is watching. That is the whole point:
//! one execution path means one place to gate, one place to explain and one place to record. A
//! capability that behaved differently for an agent than for the scheduler would be two
//! capabilities wearing one name, and only one of them would ever get audited.
//!
//! ## Three steps, and they are separate on purpose
//!
//! ```text
//! describe()   what this is           static, no arguments
//! explain()    what this call does    arguments known, nothing has happened yet
//! run()        do it                  the only step with an effect
//! ```
//!
//! `explain` exists so the user can be told before rather than after (ADR-0009), and it is a
//! distinct method rather than a flag on `run` because a "dry run" that shares a code path with
//! the real one eventually does something on a Tuesday.
//!
//! ## What comes back is never trusted
//!
//! Every [`Outcome`] is foreign text: a file somebody else wrote, a page from the internet, the
//! output of a program. It goes to the model as **data**, wrapped, in a user-role message —
//! never as an instruction and never in the system role. A page that can tell a character what
//! to do, when that character can run commands, is the whole attack.
//!
//! This is a property of *all* tool output, so there is deliberately no flag to mark some
//! results trusted. A flag would eventually be set wrongly by somebody in a hurry.

use std::collections::BTreeMap;

use epoch_kernel::{Arguments, CapabilityId, Descriptor, Explanation};

#[derive(Debug, thiserror::Error)]
pub enum CapabilityError {
    /// The arguments are unusable. Every reason at once, so a model can fix them in one go
    /// rather than discovering them one round at a time.
    #[error("{0}")]
    BadArguments(String),
    /// It ran and it did not work. The message goes back to the model, which may well recover.
    #[error("{0}")]
    Failed(String),
    /// The Trust Engine said no. Distinct from failure: nothing was attempted, and the model
    /// must be told that rather than left to conclude the machine is broken.
    #[error("not allowed: {0}")]
    Refused(String),
}

impl CapabilityError {
    /// Every problem, joined. Used where a model gets one string back.
    pub fn from_problems(problems: Vec<String>) -> Self {
        CapabilityError::BadArguments(problems.join("; "))
    }
}

/// Somewhere a result came from, addressable enough for the user to go and check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub title: String,
    pub url: String,
}

/// What a capability produced.
///
/// The content is **untrusted** — see the module docs. There is no field saying otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// What goes back to the model, as data.
    pub content: String,
    /// A durable thing this produced, if it produced one: a file written, a commit made.
    ///
    /// This is what makes a Quest's History real (ADR-0025) — a Quest that produced no
    /// evidence produced nothing, and History has to be able to say so.
    pub evidence: Option<Made>,
    /// This call **began** something that will finish later, rather than finishing (ADR-0034).
    ///
    /// `None` for every capability that answers inside its own call, which is almost all of them.
    ///
    /// ## A field rather than a new shape on the trait
    ///
    /// Whether a call is long is a fact about *this call's arguments* — a render of one frame is
    /// not a render of two hundred — so the only thing that can answer it is the capability, at
    /// the moment it looks. A `fn is_long()` beside `describe()` would be a manifest entry, and
    /// ADR-0030 already recorded what those do: *a capability typed into a manifest is one that
    /// will eventually lie.*
    ///
    /// It rides `Outcome` rather than widening the trait so the twenty capabilities that will
    /// never be long are untouched.
    pub begun: Option<Begun>,
    /// Where the content came from, when it came from somewhere addressable.
    ///
    /// Not evidence — a search leaves nothing behind, so a Quest that only searched still
    /// produced nothing (ADR-0025). These exist so an answer can be *checked*: a surface can
    /// show what was read, and the user can go and read it themselves. Structured rather than
    /// parsed back out of the text, because a surface that recovered URLs from prose would
    /// eventually cite something the character merely mentioned.
    pub sources: Vec<Source>,
}

impl Outcome {
    /// A result with nothing durable behind it. Reading something is not evidence.
    pub fn told(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            evidence: None,
            begun: None,
            sources: Vec::new(),
        }
    }

    /// This call started something. Nothing exists yet, and the content says so.
    ///
    /// The sentence is not the caller's to write: `jobs::told_it_started` owns it, because what a
    /// tool says back is prompt and this one has to refuse two invitations at once.
    pub fn begun(
        id: impl Into<String>,
        what: impl Into<String>,
        waiting_on: impl Into<String>,
    ) -> Self {
        let what = what.into();
        Self {
            content: crate::jobs::told_it_started(&what),
            evidence: None,
            begun: Some(Begun {
                id: id.into(),
                what,
                waiting_on: waiting_on.into(),
            }),
            sources: Vec::new(),
        }
    }

    /// Say where it came from. Additive, so nothing else has to know these exist.
    pub fn from(mut self, sources: Vec<Source>) -> Self {
        self.sources = sources;
        self
    }

    /// A result that left something behind.
    ///
    /// `reference` is **the thing**, in whatever terms its kind uses — a path, a command, a
    /// hash. It is separate from the sentence because a note that says "created pepe.txt" reads
    /// fine and cannot be followed: nothing downstream can open it, list it, or check it still
    /// exists. The first History Epoch wrote said `mage (capability) — created pepe.txt`, which
    /// records who was in the room rather than what came out of it.
    pub fn made(
        content: impl Into<String>,
        reference: impl Into<String>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            content: content.into(),
            evidence: Some(Made {
                reference: reference.into(),
                summary: summary.into(),
            }),
            begun: None,
            sources: Vec::new(),
        }
    }
}

/// What a capability left behind, named so it can be followed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Made {
    /// The thing itself: `src/auth.rs`, `cargo test`, a commit hash.
    pub reference: String,
    /// One line about it, for a History nobody has to decode.
    pub summary: String,
}

/// Work a call started and did not finish.
///
/// Both fields are for a person, not for the model. `what` is what a surface says a character is
/// doing; `waiting_on` is the risk ADR-0034 names — a ComfyUI that hangs leaves somebody working
/// forever, and a job that says what it is waiting on can be explained rather than guessed at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Begun {
    /// What this work is called while it runs.
    ///
    /// **Minted by the capability, not by the layer above.** The capability's own thread has to
    /// name the result when it lands, and the shell has to file it against the Quest it began
    /// under — so both need the same word, and the only one that exists before the work starts is
    /// the one the starter chose.
    pub id: String,
    /// In the words a person reads: `drawing a picture`.
    pub what: String,
    /// What it depends on: `ComfyUI at http://127.0.0.1:8188`.
    pub waiting_on: String,
}

/// Something the World can do.
///
/// Deliberately narrow. A capability that needed to know about Quests, characters or the
/// Activity Stream would have grown into the Execution Engine's job — that layer owns the
/// cross-cutting concerns, and capabilities do the work (ADR-0008).
pub trait Capability: Send + Sync {
    /// What this is. Static, and checked once when it is registered.
    fn describe(&self) -> Descriptor;

    /// What *this call* would do, said before it happens.
    ///
    /// The default builds an honest sentence from the arguments. A capability that can say
    /// something more useful — how many files match, which branch would be pushed — overrides
    /// it, and should.
    fn explain(&self, arguments: &Arguments) -> Result<Explanation, CapabilityError> {
        let descriptor = self.describe();
        let problems = arguments.check(&descriptor.parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        Ok(Explanation::of(
            &descriptor,
            describe_call(&descriptor, arguments),
        ))
    }

    /// Do it. The only method with an effect.
    ///
    /// Called after the Trust Engine has allowed it, and never instead of `explain` — the
    /// caller has already said out loud what this would do.
    fn run(&self, arguments: &Arguments) -> Result<Outcome, CapabilityError>;

    /// Do it, saying what is happening while it happens.
    ///
    /// Most capabilities are over before there is anything to report — a file is read or it is
    /// not — so the default simply runs and says nothing, and implementing this is opt-in.
    ///
    /// It exists for the ones where **the waiting is the experience**: a build, a test run, a
    /// clone of something large. The alternative was a spinner, and a spinner is a picture of
    /// waiting rather than an account of it. Nothing here changes what a capability *does* or
    /// what it returns; `Outcome` is still the result, and the lines are observations along the
    /// way, never evidence (ADR-0025 — evidence is what a run left behind).
    fn run_watched(
        &self,
        arguments: &Arguments,
        _saying: &mut dyn FnMut(&str),
    ) -> Result<Outcome, CapabilityError> {
        self.run(arguments)
    }
}

/// A readable sentence for a call nobody wrote a better one for.
///
/// `read_file(path: src/main.rs)` rather than a JSON blob: the user is being asked to approve
/// this, and an explanation nobody reads is not an explanation.
fn describe_call(descriptor: &Descriptor, arguments: &Arguments) -> String {
    if arguments.is_empty() {
        return descriptor.id.to_string();
    }
    let listed: Vec<String> = arguments
        .iter()
        .map(|(name, value)| format!("{name}: {value}"))
        .collect();
    format!("{}({})", descriptor.id, listed.join(", "))
}

/// Everything this build can do.
///
/// Registration validates the descriptor, so a capability that contradicts itself cannot be
/// reached at all — rather than being discovered at the moment somebody is asked to approve it.
#[derive(Default)]
pub struct CapabilityRegistry {
    /// Ordered by id, so every listing derived from this is stable across runs.
    entries: BTreeMap<CapabilityId, Box<dyn Capability>>,
    /// Capabilities that were offered and refused, with the reason. Reported, never silent —
    /// a capability that vanished without explanation gets blamed on the model for a week.
    problems: Vec<String>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Offer a capability. Refused, with reasons, if its descriptor does not hold together.
    pub fn register(&mut self, capability: Box<dyn Capability>) {
        let descriptor = capability.describe();
        let problems = descriptor.problems();
        if !problems.is_empty() {
            self.problems.extend(problems);
            return;
        }
        if self.entries.contains_key(&descriptor.id) {
            self.problems.push(format!(
                "two capabilities claim the name '{}'; the first one keeps it",
                descriptor.id
            ));
            return;
        }
        self.entries.insert(descriptor.id, capability);
    }

    pub fn get(&self, id: &CapabilityId) -> Option<&dyn Capability> {
        self.entries.get(id).map(|c| c.as_ref())
    }

    /// Every descriptor, in a stable order.
    pub fn describe_all(&self) -> Vec<Descriptor> {
        self.entries.values().map(|c| c.describe()).collect()
    }

    /// The capabilities a character asked for, and which actually exist.
    ///
    /// **This is the tool set for a turn.** Authored intent selects it (ADR-0026), rather than
    /// every capability being offered to every model on every message and the model choosing
    /// from a menu it should not have been shown.
    ///
    /// A request nobody can meet simply is not here. The request stays in the character's file
    /// — it says what they are *for* — and the surface can show it as unmet.
    pub fn requested<'a>(&'a self, wanted: impl IntoIterator<Item = &'a str>) -> Vec<Descriptor> {
        let mut found: Vec<Descriptor> = wanted
            .into_iter()
            .filter_map(|name| CapabilityId::new(name).ok())
            .filter_map(|id| self.entries.get(&id))
            .map(|c| c.describe())
            .collect();
        found.sort_by(|a, b| a.id.cmp(&b.id));
        found.dedup_by(|a, b| a.id == b.id);
        found
    }

    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl std::fmt::Debug for CapabilityRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CapabilityRegistry")
            .field("entries", &self.entries.keys().collect::<Vec<_>>())
            .field("problems", &self.problems)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use epoch_kernel::{Effect, Parameter, Reversal, Risk, Value, ValueKind};

    /// A capability that only looks — the shape Explorer mode allows.
    struct Reader;

    impl Capability for Reader {
        fn describe(&self) -> Descriptor {
            Descriptor::observing(
                CapabilityId::new("read_file").unwrap(),
                "Read a file from the project.",
            )
            .taking([Parameter::required(
                "path",
                ValueKind::Text,
                "Path within the project.",
            )])
        }

        fn run(&self, arguments: &Arguments) -> Result<Outcome, CapabilityError> {
            let path = arguments
                .text("path")
                .map_err(CapabilityError::BadArguments)?;
            Ok(Outcome::told(format!("contents of {path}")))
        }
    }

    /// A capability whose descriptor does not hold together.
    struct Liar;

    impl Capability for Liar {
        fn describe(&self) -> Descriptor {
            Descriptor::acting(
                CapabilityId::new("wipe").unwrap(),
                "Remove things.",
                [Effect::Deletes],
                Reversal::NothingToUndo,
            )
        }

        fn run(&self, _arguments: &Arguments) -> Result<Outcome, CapabilityError> {
            unreachable!("a refused capability is never reachable")
        }
    }

    fn id(raw: &str) -> CapabilityId {
        CapabilityId::new(raw).unwrap()
    }

    #[test]
    fn a_capability_that_contradicts_itself_never_becomes_reachable() {
        // Caught at registration rather than at the moment somebody is asked to approve it.
        let mut registry = CapabilityRegistry::new();
        registry.register(Box::new(Liar));
        assert!(registry.is_empty());
        assert!(registry.problems()[0].contains("nothing to undo"));
        assert!(registry.get(&id("wipe")).is_none());
    }

    #[test]
    fn two_capabilities_cannot_claim_one_name() {
        let mut registry = CapabilityRegistry::new();
        registry.register(Box::new(Reader));
        registry.register(Box::new(Reader));
        assert_eq!(registry.len(), 1);
        assert!(registry.problems()[0].contains("two capabilities claim"));
    }

    #[test]
    fn explaining_comes_before_running_and_refuses_the_same_things() {
        let reader = Reader;
        // Nothing to run on: caught while explaining, so the user is never asked to approve a
        // call that could not have worked.
        let empty = Arguments::new();
        assert!(matches!(
            reader.explain(&empty),
            Err(CapabilityError::BadArguments(ref why)) if why.contains("'path' is required")
        ));

        let args = Arguments::new().with("path", Value::Text("src/main.rs".into()));
        let explained = reader.explain(&args).unwrap();
        assert_eq!(explained.what, "read_file(path: src/main.rs)");
        assert_eq!(explained.risk, Risk::None);
        assert_eq!(
            explained.line(),
            "read_file(path: src/main.rs) — nothing to undo"
        );
    }

    #[test]
    fn a_character_gets_the_capabilities_they_asked_for_and_no_others() {
        // The tool set for a turn is authored intent, not a menu of everything that exists.
        let mut registry = CapabilityRegistry::new();
        registry.register(Box::new(Reader));

        let offered = registry.requested(["read_file", "web_search"]);
        assert_eq!(
            offered.len(),
            1,
            "a request nobody can meet is simply not offered"
        );
        assert_eq!(offered[0].id.as_str(), "read_file");

        assert!(registry.requested(["web_search"]).is_empty());
        // A name a model could never have produced does not become a lookup for one.
        assert!(registry.requested(["../etc/passwd"]).is_empty());
    }

    #[test]
    fn asking_twice_offers_once() {
        let mut registry = CapabilityRegistry::new();
        registry.register(Box::new(Reader));
        assert_eq!(registry.requested(["read_file", "read_file"]).len(), 1);
    }

    #[test]
    fn running_returns_untrusted_content_and_says_whether_anything_was_left_behind() {
        let args = Arguments::new().with("path", Value::Text("src/main.rs".into()));
        let outcome = Reader.run(&args).unwrap();
        assert_eq!(outcome.content, "contents of src/main.rs");
        // Reading is not evidence. A Quest that only read things produced nothing (ADR-0025).
        assert_eq!(outcome.evidence, None);
    }
}
