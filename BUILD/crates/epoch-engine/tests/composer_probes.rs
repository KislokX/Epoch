//! Three probes for the personality of a composed turn: **obedience · restraint · greeting**.
//!
//! ## Why this exists at all
//!
//! The block that tells a character what it can do has broken twice, in opposite directions,
//! and both times the break was invisible to every unit test. Written weakly, a character stops
//! reaching for tools it has and answers from memory. Written strongly — or delivered on the
//! wrong channel — it stops being a description and becomes a second request: asked only to
//! clone a repository, the character cloned it, read the README and summarised it. Nobody asked.
//!
//! ADR-0012 records those measurements as a table somebody produced by hand, once. This is that
//! table as something anyone can run again, because a measurement that cannot be repeated is a
//! memory, and the next person to touch that block will otherwise rediscover the same two ditches.
//!
//! ## The three questions, and why all three
//!
//! | probe | asks | the failure it catches |
//! |---|---|---|
//! | **obedience** | something only a tool can answer | a character that answers from memory instead of looking |
//! | **restraint** | something already finished | a character that keeps going, doing work nobody asked for |
//! | **greeting** | "hola" | a character that answers a greeting by running things |
//!
//! **No number here may be quoted alone.** A wording that buys restraint by giving back refusal
//! is the old bug wearing a new coat, and one that buys obedience by making the character
//! restless is the other one. They move together or the change is not an improvement.
//!
//! ## What is real and what is not
//!
//! The model, the composer, the turn loop and the capability descriptions are all real — this
//! drives the same `turn::run` a conversation does. The capabilities themselves are stand-ins
//! that record being called and answer plausibly, because the question is *whether the character
//! reached*, and nothing here should touch a disk to find out.
//!
//! ```text
//! cargo test -p epoch-engine --test composer_probes -- --ignored --nocapture
//! ```

use std::sync::{Arc, Mutex};

use epoch_engine::capability::{Capability, CapabilityError, CapabilityRegistry, Outcome};
use epoch_engine::context::Ingredients;
use epoch_engine::provider::{KeepLoaded, Ollama, Request};
use epoch_engine::trust::TrustStore;
use epoch_engine::TrustEngine;
use epoch_kernel::{
    Arguments, Autonomy, Budget, CharacterArchetype, CharacterDefinition, CharacterId, Descriptor,
    Entry, Explanation, IdleBehavior, Parameter, PresenceProfile, Quest, QuestId, Reversal,
    ValueKind,
};

/// How many times each probe is run.
///
/// Three, because a model is not deterministic and one run of anything proves nothing. ADR-0012's
/// own table is written as "3 of 3" for the same reason.
const RUNS: usize = 3;

/// Which model these probes speak to.
///
/// Overridable, because the answer is a property of a model as much as of the wording — the
/// channel finding in ADR-0012 was true of one model and might not be true of the next.
fn model() -> String {
    std::env::var("EPOCH_PROBE_MODEL").unwrap_or_else(|_| "gemma4:12b".into())
}

/// A capability that records being reached for and answers plausibly.
///
/// It never touches anything. The probes ask whether the character *reached*, and a stand-in
/// that answers convincingly measures that without a project, a disk or a repository.
struct Watched {
    descriptor: Descriptor,
    says: String,
    reached: Arc<Mutex<Vec<String>>>,
}

impl Capability for Watched {
    fn describe(&self) -> Descriptor {
        self.descriptor.clone()
    }

    fn explain(&self, _arguments: &Arguments) -> Result<Explanation, CapabilityError> {
        Ok(Explanation::of(&self.descriptor, "a probe"))
    }

    fn run(&self, _arguments: &Arguments) -> Result<Outcome, CapabilityError> {
        self.reached
            .lock()
            .expect("probe lock")
            .push(self.descriptor.id.to_string());
        Ok(Outcome::told(self.says.clone()))
    }
}

fn reading(id: &str, summary: &str, parameter: &str) -> Descriptor {
    Descriptor::observing(epoch_kernel::CapabilityId::new(id).unwrap(), summary).taking([
        Parameter::required(parameter, ValueKind::Text, "what to look at"),
    ])
}

/// The tools a character has in these probes: one that reads, one that runs.
fn registry(reached: &Arc<Mutex<Vec<String>>>) -> (CapabilityRegistry, Vec<Descriptor>) {
    let list = reading(
        "list_files",
        "List the files and folders in one folder of the project.",
        "path",
    );
    let read = reading(
        "read_file",
        "Read a text file from the project. Optionally a window of lines.",
        "path",
    );
    let run = Descriptor::acting(
        epoch_kernel::CapabilityId::new("run_command").unwrap(),
        "Run a development command in the project — git, cargo, npm, node, python.",
        [epoch_kernel::Effect::Executes],
        Reversal::Permanent,
    )
    .taking([Parameter::required(
        "command",
        ValueKind::Text,
        "The whole command.",
    )]);

    let mut registry = CapabilityRegistry::default();
    for (descriptor, says) in [
        (list.clone(), "src/  README.md  Cargo.toml"),
        (read.clone(), "# whallet\\n\\nA budgeting app."),
        (run.clone(), "Cloning into 'whallet'... done."),
    ] {
        registry.register(Box::new(Watched {
            descriptor,
            says: says.into(),
            reached: Arc::clone(reached),
        }));
    }
    (registry, vec![list, read, run])
}

fn mage() -> CharacterDefinition {
    CharacterDefinition {
        id: CharacterId::new("mage").unwrap(),
        name: "Mage".into(),
        archetype: CharacterArchetype::Researcher,
        role: "Turns goals into clean designs".into(),
        worlds: [("default".to_string(), Default::default())].into(),
        prompt: "You explore the solution space before committing, and you name tradeoffs.".into(),
        skills: Default::default(),
        requested_capabilities: Default::default(),
        mind: None,
        appearance: None,
        draws_in: None,
        speaks_with: None,
        sounds_like: None,
        presence: PresenceProfile {
            authored_home: None,
            idle: vec![IdleBehavior {
                activity: "reading".into(),
                seconds: 5,
            }],
        },
    }
}

fn quest(intent: &str, exchanges: &[(&str, &str)]) -> Quest {
    let mut quest = Quest::inaugurate(
        QuestId::from_raw("q_probe"),
        "default",
        CharacterId::new("mage").unwrap(),
        intent,
        intent,
        Default::default(),
        0,
    );
    for (at, (said, answered)) in exchanges.iter().enumerate() {
        quest.record(
            at as u64,
            Entry::Said {
                content: (*said).to_owned(),
                attachments: Vec::new(),
                images: Vec::new(),
            },
        );
        quest.record(
            at as u64,
            Entry::Answered {
                character: CharacterId::new("mage").unwrap(),
                content: (*answered).to_owned(),
                pace: None,
            },
        );
    }
    quest
}

/// Run one composed turn and report which capabilities the character reached for.
fn probe(
    quest: &Quest,
    tools: Vec<Descriptor>,
    registry: &CapabilityRegistry,
) -> (Vec<String>, String) {
    let ingredients = Ingredients {
        tools,
        working_in: Some("C:/projects/whallet".into()),
        ..Ingredients::default()
    };
    let (conversation, _report) =
        epoch_engine::compose_turn(quest, &mage(), &ingredients, Budget::default());

    let store = TrustStore::default();
    let who = CharacterId::new("mage").unwrap();
    // Auto, because the question is what the character *chooses* to do. A mode that stops to ask
    // would measure Trust rather than the turn.
    let trust = TrustEngine::new(&store, &who, "default", Autonomy::Auto);

    let request = Request {
        model: model(),
        conversation,
        keep_loaded: KeepLoaded::For(std::time::Duration::from_secs(600)),
        parameters: epoch_kernel::Parameters {
            // Zero, so a difference between two runs is a difference in the words rather than in
            // the sampler.
            temperature: Some(0.0),
            ..Default::default()
        },
        tuning: Default::default(),
        tools: request_tools(registry),
        // The built-in bound: a test is not the place to decide how patient this machine is.
        most_rounds: None,
    };

    let done = epoch_engine::turn::run(
        &Ollama::default(),
        &trust,
        registry,
        request,
        &|| false,
        &mut |_| {},
        &mut |_| {},
    );

    match done {
        Ok(completed) => (
            completed
                .evidence
                .iter()
                .map(|made| made.summary.clone())
                .collect(),
            completed.text,
        ),
        Err(err) => (Vec::new(), format!("<failed: {err}>")),
    }
}

fn request_tools(registry: &CapabilityRegistry) -> Vec<Descriptor> {
    registry.describe_all()
}

/// Which capabilities were reached for, across `RUNS` runs of one scenario.
///
/// Returns how many runs reached for anything **beyond** `expected`. For a probe where
/// reaching is the healthy answer, pass what it is allowed to reach for; for one where
/// stillness is the answer, pass nothing.
fn tally(name: &str, quest: &Quest, allowed: &[&str]) -> usize {
    let reached = Arc::new(Mutex::new(Vec::new()));
    let (registry, tools) = registry(&reached);
    let mut acted = 0;

    for run in 1..=RUNS {
        reached.lock().expect("probe lock").clear();
        let (_evidence, said) = probe(quest, tools.clone(), &registry);
        let calls = reached.lock().expect("probe lock").clone();
        if calls.iter().any(|c| !allowed.contains(&c.as_str())) {
            acted += 1;
        }
        println!(
            "  {name} run {run}: {} — {}",
            if calls.is_empty() {
                "reached for nothing".to_string()
            } else {
                format!("reached for {}", calls.join(", "))
            },
            said.lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(90)
                .collect::<String>()
        );
    }
    acted
}

#[test]
#[ignore = "needs Ollama serving EPOCH_PROBE_MODEL (gemma4:12b by default); three real turns"]
fn obedience_a_character_looks_instead_of_answering_from_memory() {
    // Asked something only a tool can answer, against a Chronicle long enough to drown a system
    // message — the condition under which the capability block was found to lose (ADR-0012).
    let chatter: Vec<(&str, &str)> = (0..12)
        .map(|_| {
            (
                "what do you think of the plan?",
                "I think the plan is reasonable and I would keep it simple.",
            )
        })
        .collect();
    let mut quest = quest("work on whallet", &chatter);
    quest.record(
        99,
        Entry::Said {
            content: "¿qué archivos hay en la raíz del proyecto?".into(),
            attachments: Vec::new(),
            images: Vec::new(),
        },
    );

    // Anything at all: the question is whether it looks, not what it looks at.
    let acted = tally("obedience", &quest, &[]);
    println!("OBEDIENCE: looked in {acted} of {RUNS}");
    assert_eq!(
        acted, RUNS,
        "a character asked something only a tool can answer must look, every time"
    );
}

#[test]
#[ignore = "needs Ollama serving EPOCH_PROBE_MODEL (gemma4:12b by default); three real turns"]
fn restraint_a_character_stops_when_the_work_is_done() {
    // The other ditch, and it happens **inside one turn**: the clone succeeds, and the character
    // goes on to read the README and summarise it because a capability block written to defeat
    // refusal reads as a second request (ADR-0012).
    //
    // Measured as "reached for anything besides the one thing asked for", because the first
    // version of this probe ended the Chronicle with the character's own reply — nothing had
    // been asked, so it reached for nothing, and the probe congratulated itself for measuring
    // restraint it had made impossible to fail.
    let mut quest = quest("cloná el repo de whallet", &[]);
    quest.record(
        1,
        Entry::Said {
            content: "cloná el repo https://github.com/example/whallet y nada más".into(),
            attachments: Vec::new(),
            images: Vec::new(),
        },
    );

    let overreached = tally("restraint", &quest, &["run_command"]);
    println!("RESTRAINT: went further in {overreached} of {RUNS} (0 is the healthy number)");
    assert_eq!(
        overreached, 0,
        "only the clone was asked for; anything else is work nobody wanted"
    );
}

#[test]
#[ignore = "needs Ollama serving EPOCH_PROBE_MODEL (gemma4:12b by default); three real turns"]
fn a_greeting_is_answered_and_not_worked_on() {
    // The one a person notices first. "hola" is not a task, and a character that answers it by
    // listing a directory has stopped being somebody you can talk to.
    let mut quest = quest("hola", &[]);
    quest.record(
        1,
        Entry::Said {
            content: "hola".into(),
            attachments: Vec::new(),
            images: Vec::new(),
        },
    );

    let acted = tally("greeting", &quest, &[]);
    println!("GREETING: acted in {acted} of {RUNS} (0 is the healthy number)");
    assert_eq!(acted, 0, "a greeting is answered, not worked on");
}
