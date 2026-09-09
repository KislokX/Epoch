//! One model, put through everything, and what came out.
//!
//! ## What a card is
//!
//! A row of the table: one model, one configuration, one machine, one suite version. It carries
//! the speed readings from [`crate::bench`], the judged trials from [`crate::trials`], and enough
//! about *where and how* it was measured that a card from another day can be compared with it
//! honestly — or refused, which is the more common right answer.
//!
//! **A number without its conditions is not a measurement.** A card that said *45 tok/s* and not
//! which card, which build, which context and which suite would be a fact about nothing.
//!
//! ## The two phases live here as two functions
//!
//! [`standard`] runs every model the same way and is the only thing whose numbers may be put side
//! by side. The Limit phase is per model and answers a different question — how far this one goes
//! — so it produces its own cards and never shares a table with a Standard one.

use serde::{Deserialize, Serialize};

use crate::bench::{Ask, Measured};
use crate::trials::{Answered, Blind, Outcome, ToolResult, Trial};

/// Where and how a card was measured.
///
/// Every field here is the answer to *would this number mean the same thing on another day*, and
/// every one of them is read rather than typed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conditions {
    /// Which questions. A card from suite 1 and one from suite 2 are not rows of one table.
    pub suite: u32,
    /// The card, in the machine's own words. Empty where nothing could be read — never a guess.
    pub gpu: String,
    /// The runtime's own build string, because a release that changes kernels changes the answer.
    pub build: String,
    pub runtime: String,
    pub context: u32,
    /// What the model was told beyond context: flash attention, speculation.
    pub tuning: crate::models::tuning::Tuning,
    pub at: u64,
}

/// One model's whole result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub model: String,
    pub conditions: Conditions,
    /// How fast, across the runs that were made.
    pub speed: Measured,
    /// Every trial, with what the model said and how it was judged.
    pub answers: Vec<Answered>,
}

impl Card {
    /// How much of what it *answered* was right. `None` where it answered nothing of that kind.
    ///
    /// **This is correctness alone**, and on its own it is the misleading half — a model that
    /// answered one trial of three and got it right scores 100% here. Read [`Card::column`]
    /// instead unless one number is genuinely all that is wanted; it is derived from the same
    /// counts, so the two cannot disagree.
    pub fn score(&self, kind: Kind) -> Option<f64> {
        self.column(kind).correctness
    }

    /// One column of the table: three rates and the counts they came from.
    ///
    /*
        **`100% (4/5)` was one number pretending to be one fact, and it is three.**

        The owner's correction, and it is right: a model that answered four of five reasoning
        trials and got all four right has done two separable things. It was **accurate** on
        everything it said, and it **finished** four fifths of what it was given. Collapsing those
        into one figure means an empty answer either counts as wrong — which it is not, it was
        never given — or costs nothing, which is worse, because a task nobody completed is not a
        task that went well.

        So:

        - **correctness** — of what it answered, how much was right. `4/4 = 100%`.
        - **completion** — of what it was asked, how much it answered at all. `4/5 = 80%`.
        - **effective** — of what it was asked, how much came back right. `4/5 = 80%`.

        `effective` is the one to rank on and it is deliberately *not* the product of the other
        two as a rounded pair: it is computed from the same counts, so three rates that must agree
        are derived from one measurement rather than from each other.

        The counts travel too, because a rate over five trials and the same rate over fifty are
        not the same evidence, and no surface may collapse them again.
    */
    pub fn column(&self, kind: Kind) -> Column {
        let every = crate::suite::all();
        let mine: Vec<&Answered> = self
            .answers
            .iter()
            .filter(|it| {
                every
                    .iter()
                    .find(|trial| trial.id() == it.trial)
                    .map(Kind::of_trial)
                    == Some(kind)
            })
            .collect();

        // A trial's score is fractional for the kinds that are judged in parts — a tool call is
        // five or six separate facts — so `correct` is a sum and not a tally.
        let scored: Vec<f64> = mine.iter().filter_map(|it| it.outcome.score()).collect();
        let asked = mine.len();
        let answered = scored.len();
        let correct: f64 = scored.iter().sum();

        Column {
            kind,
            correct,
            answered,
            asked,
            correctness: (answered > 0).then(|| correct / answered as f64),
            completion: (asked > 0).then(|| answered as f64 / asked as f64),
            effective: (asked > 0).then(|| correct / asked as f64),
        }
    }

    /// Every column, in the order a table reads them.
    pub fn columns(&self) -> Vec<Column> {
        [Kind::Reasoning, Kind::Coding, Kind::Following, Kind::Tools]
            .into_iter()
            .map(|kind| self.column(kind))
            .collect()
    }

    /// Tests passed and tests attempted, across every coding trial.
    pub fn tests(&self) -> (u32, u32) {
        self.answers
            .iter()
            .fold((0, 0), |(pass, all), one| match one.outcome {
                Outcome::Coding { passed, total, .. } => (pass + passed, all + total),
                _ => (pass, all),
            })
    }
}

/// One column of a card: three rates, and the counts they were derived from.
///
/// **Three, because `100% (4/5)` is three facts wearing one number.** Accuracy on what was said
/// and completion of what was asked are separable, and a surface that collapses them either
/// scores an unanswered trial as wrong — it was not, it was never given — or charges nothing for
/// it, which is worse: a task nobody completed did not go well.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    pub kind: Kind,
    /// Sum of the per-trial scores. A **sum, not a tally** — a tool call is judged in five or six
    /// separate parts, so a trial can be four fifths right.
    pub correct: f64,
    /// How many trials of this kind produced an answer at all.
    pub answered: usize,
    /// How many were asked.
    pub asked: usize,
    /// `correct / answered`. `None` where it answered nothing — never zero.
    pub correctness: Option<f64>,
    /// `answered / asked`. `None` where none were asked.
    pub completion: Option<f64>,
    /// `correct / asked`. **The one to rank on**, and computed from the counts rather than from
    /// the other two, so three numbers that must agree come from one measurement.
    pub effective: Option<f64>,
}

/// Which family a trial belongs to, for a table with four columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Reasoning,
    Coding,
    Following,
    Tools,
}

impl Kind {
    /// Which column a trial belongs to, whatever happened to it.
    ///
    /// This is the half [`Kind::of`] cannot answer: a refusal belongs to no column, so a card
    /// that only knew outcomes could not say how many coding trials it had *asked*.
    pub fn of_trial(trial: &Trial) -> Self {
        match trial {
            Trial::Reasoning { .. } => Kind::Reasoning,
            Trial::Coding { .. } => Kind::Coding,
            Trial::Following { .. } => Kind::Following,
            Trial::Tools { .. } | Trial::Agentic { .. } => Kind::Tools,
        }
    }

    /// `None` for an outcome that never happened — a refusal belongs to no column.
    pub fn of(outcome: &Outcome) -> Option<Self> {
        match outcome {
            Outcome::Reasoning { .. } => Some(Kind::Reasoning),
            Outcome::Coding { .. } => Some(Kind::Coding),
            Outcome::Following { .. } => Some(Kind::Following),
            Outcome::Tools(_) | Outcome::Agentic { .. } => Some(Kind::Tools),
            Outcome::Refused { .. } => None,
        }
    }
}

/// Ask a server one trial and judge the answer.
///
/// Off in its own function because the three ways a trial is judged — read, run, inspect — all
/// begin with the same request, and a benchmark where the tool trials were asked differently from
/// the rest would be comparing two things.
pub fn attempt(at: &str, model: &str, trial: &Trial) -> Answered {
    let tools = match trial {
        Trial::Tools { offered, .. } | Trial::Agentic { offered, .. } => Some(
            offered
                .iter()
                .map(|it| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": it.name,
                            "description": it.description,
                            "parameters": it.arguments,
                        }
                    })
                })
                .collect::<Vec<_>>(),
        ),
        _ => None,
    };

    let said = match ask(at, model, trial.prompt(), tools.as_deref()) {
        Ok(said) => said,
        Err(why) => {
            return Answered {
                trial: trial.id().to_owned(),
                model: model.to_owned(),
                said: String::new(),
                outcome: Outcome::Refused { why },
            }
        }
    };

    let text = said["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or_default()
        .to_owned();

    if let Some(refused) = read_or_refuse(&said) {
        return Answered {
            trial: trial.id().to_owned(),
            model: model.to_owned(),
            said: text,
            outcome: refused,
        };
    }

    let outcome = match trial {
        Trial::Reasoning { .. } | Trial::Following { .. } => crate::trials::judge(trial, &text),
        Trial::Coding { tests, total, .. } => {
            let code = crate::ran::code_in(&text, crate::suite::CODING_LANGUAGE);
            match crate::ran::run(&code, tests, crate::suite::CODING_LANGUAGE) {
                Some(ran) => Outcome::Coding {
                    // All of them or none: the interpreter stops at the first failed assertion,
                    // so a partial count would be invented. What is kept instead is what it said,
                    // which names the assertion that stopped it.
                    passed: if ran.passed { *total } else { 0 },
                    total: *total,
                    note: (!ran.passed).then_some(ran.said),
                },
                // Not run, which is not failed. A machine without the interpreter is an ordinary
                // machine and the model has nothing to answer for.
                None => Outcome::Refused {
                    why: "this machine cannot run the language the trial is written in".to_owned(),
                },
            }
        }
        Trial::Tools {
            expect, offered, ..
        } => Outcome::Tools(inspect(&said, &text, expect, offered)),
        // Its own conversation, because one request cannot carry several turns.
        Trial::Agentic {
            prompt,
            offered,
            steps,
            finally,
            ..
        } => converse(at, model, prompt, offered, steps, finally),
    };

    Answered {
        trial: trial.id().to_owned(),
        model: model.to_owned(),
        said: text,
        outcome,
    }
}

/// Whether the model answered at all.
///
/*
    **An answer that never arrived is unanswered, not wrong.**

    Measured: `gemma4:12b` asked what day a 45-day task starting on a Tuesday ends on spent every
    one of its 700 tokens in `reasoning_content` and returned `content: ""` with
    `finish_reason: "length"`. Reading that as a wrong answer scored a capable model at zero for
    something the harness did — the same invention as a gauge with nothing behind it reading `0`,
    and it would have ranked every thinking model below every model that answers immediately, for
    a reason that is not about either of them.

    The budget was raised too (`suite::TRIAL_TOKENS`), and that fixes this model rather than the
    class: the same session watched the same model think for 12,953 characters about a twenty-word
    answer and stop with nothing. So the honest reading has to exist whatever the budget is.

    Not for a tool trial, where an empty answer beside a call is exactly right — the model reached
    for something instead of talking, which is the thing being measured.

    Off in its own function so it can be tested against the wire shape that produced it, rather
    than against a live server that would have to be persuaded to think itself out of room.
*/
fn read_or_refuse(said: &serde_json::Value) -> Option<Outcome> {
    let text = said["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or_default();
    let ran_out = said["choices"][0]["finish_reason"].as_str() == Some("length");
    let reached = said["choices"][0]["message"]["tool_calls"][0].is_object();
    if !text.trim().is_empty() || !ran_out || reached {
        return None;
    }
    let thought = said["choices"][0]["message"]["reasoning_content"]
        .as_str()
        .is_some_and(|it| !it.trim().is_empty());
    Some(Outcome::Refused {
        why: if thought {
            format!(
                "it spent all {} tokens thinking and never began an answer",
                crate::suite::TRIAL_TOKENS
            )
        } else {
            format!(
                "it ran out of room after {} tokens",
                crate::suite::TRIAL_TOKENS
            )
        },
    })
}

/// Several turns of tool use, played against a script.
///
/*
    **A conversation, because one request cannot carry several turns.** The model is asked, its
    call is inspected against the step that was expected, the harness answers with what that step
    says — sometimes an error, on purpose — and the exchange goes round again.

    Two things it measures that a single call cannot:

    - **carrying a result forward**, because step two's expected arguments come from step one's
      answer and there is nowhere else to get them;
    - **stopping**, which is the half everybody forgets. A script with no steps is the *should not
      call anything* case, and a model that reaches for a tool to add two numbers has failed
      something worth failing.

    The rounds are bounded at one more than the script. A model that keeps calling after the work
    is done has not stopped correctly, and letting it loop would measure patience.
*/
fn converse(
    at: &str,
    model: &str,
    prompt: &str,
    offered: &[crate::trials::Tool],
    script: &[crate::trials::Turn],
    finally: &[crate::trials::Check],
) -> Outcome {
    let declared: Vec<serde_json::Value> = offered
        .iter()
        .map(|it| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": it.name,
                    "description": it.description,
                    "parameters": it.arguments,
                }
            })
        })
        .collect();

    let mut talk = vec![serde_json::json!({ "role": "user", "content": prompt })];
    let mut steps: Vec<ToolResult> = Vec::new();
    let mut last_said = String::new();
    let mut stopped_correctly = false;

    for round in 0..=script.len() {
        let said = match ask_with(at, model, &talk, Some(&declared)) {
            Ok(said) => said,
            Err(why) => {
                return Outcome::Agentic {
                    steps,
                    expected: script.len(),
                    stopped_correctly: false,
                    finally: finally
                        .iter()
                        .map(|_| Err(format!("the server refused: {why}")))
                        .collect(),
                    note: Some(why),
                }
            }
        };
        let message = &said["choices"][0]["message"];
        let text = message["content"].as_str().unwrap_or_default().to_owned();
        let reached = message["tool_calls"][0].is_object();

        match script.get(round) {
            // A call was expected here.
            Some(turn) => {
                if !reached {
                    // It stopped early. The steps it never reached score zero, which
                    // `Outcome::score` supplies from `expected`.
                    last_said = text;
                    break;
                }
                let want = crate::trials::Expected {
                    tool: turn.tool.clone(),
                    arguments: turn.arguments.clone(),
                    answer: None,
                };
                steps.push(inspect(&said, &text, &want, offered));

                // Play the script's answer back, whatever the model actually called — the point
                // of a fixed script is that every model meets the same situation, including the
                // error one.
                let id = message["tool_calls"][0]["id"]
                    .as_str()
                    .unwrap_or("call")
                    .to_owned();
                talk.push(message.clone());
                talk.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": id,
                    "content": turn.responds,
                }));
            }
            // Nothing more should be called.
            None => {
                stopped_correctly = !reached;
                last_said = text;
                break;
            }
        }
    }

    Outcome::Agentic {
        expected: script.len(),
        stopped_correctly,
        finally: finally.iter().map(|it| it.against(&last_said)).collect(),
        note: (!last_said.is_empty()).then_some(last_said),
        steps,
    }
}

/// The five things worth knowing about how a model reached for a tool.
fn inspect(
    said: &serde_json::Value,
    text: &str,
    expect: &crate::trials::Expected,
    offered: &[crate::trials::Tool],
) -> ToolResult {
    let mut result = ToolResult::default();

    // Native first: that is what a Provider actually reads, and a model that emits one has done
    // the thing being measured.
    let native = said["choices"][0]["message"]["tool_calls"][0].clone();
    let (name, arguments, native_call) = if native.is_object() {
        (
            native["function"]["name"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            native["function"]["arguments"].clone(),
            true,
        )
    } else {
        /*
            **A written call is a call, and it is not the same call.** `CLAUDE.md` already holds
            that rule for turns — a model whose chat template does not emit `tool_calls` writes
            the JSON into its answer instead, and Epoch reads it. A benchmark that ignored those
            would report a model as toolless on one runtime and capable on another.

            But it is recorded as what it is: the note says the call was written rather than
            emitted, because the difference decides whether every backend can use it.
        */
        match written(text) {
            Some((name, arguments)) => {
                result.note = Some("written into the answer rather than emitted".to_owned());
                (name, arguments, false)
            }
            None => return result,
        }
    };

    result.called = true;
    result.right_tool = name == expect.tool;
    if !native_call {
        // Already noted above; kept so a reader of the struct alone can tell.
    }

    // Arguments arrive as a JSON string from most servers and as an object from some.
    let given: serde_json::Value = match &arguments {
        serde_json::Value::String(raw) => {
            serde_json::from_str(raw).unwrap_or(serde_json::Value::Null)
        }
        other => other.clone(),
    };

    // The shape the tool declared, checked against what arrived: every required property present,
    // and every property that arrived declared.
    if let Some(tool) = offered.iter().find(|it| it.name == name) {
        result.valid_shape = fits(&given, &tool.arguments);
    }

    result.right_arguments = expect
        .arguments
        .iter()
        .all(|(key, want)| given.get(key).is_some_and(|got| same(got, want)));

    // Nothing is executed here: these tools are the benchmark's own and answering them is
    // arithmetic. A call that named the right tool with arguments that fit its schema is a call
    // that would have run.
    result.ran = result.right_tool && result.valid_shape;
    result.right_answer = expect.answer.as_ref().map(|want| {
        result.ran
            && result.right_arguments
            && answer_of(&name, &given).is_some_and(|got| got.contains(want.as_str()))
    });
    result
}

/// What one of the benchmark's own tools would answer.
///
/// Two of them, and only the one with arithmetic behind it has a checkable answer. A tool whose
/// result is prose has `None` in the trial, and the fifth question is not asked rather than
/// answered generously.
fn answer_of(name: &str, given: &serde_json::Value) -> Option<String> {
    match name {
        "convert_units" => {
            /*
                **`as_number`, not `as_f64` — and the difference was a failing test.**

                `same` was written to accept `12` and `"12"` as one number, because a benchmark
                that failed a model for a server's JSON habits would be measuring the server. Then
                this function read the same field with `as_f64`, which refuses a string — so a
                call whose arguments were judged **right** produced no answer at all, and the
                fifth question came back false.

                One fact, two readings, agreeing by luck for every server that sends a number.
                The rule this repository already has, committed in the same file as the tolerance
                it contradicts.
            */
            let value = as_number(given.get("value")?)?;
            let from = given.get("from")?.as_str()?.to_lowercase();
            let to = given.get("to")?.as_str()?.to_lowercase();
            let miles = from.starts_with("mile");
            let kilometres = to.starts_with("kilom") || to.starts_with("km");
            (miles && kilometres).then(|| format!("{:.2}", value * 1.609_344))
        }
        _ => None,
    }
}

/// Whether arguments fit a JSON Schema, to the depth this benchmark needs.
///
/// **Not a JSON Schema implementation**, and saying so is the point: what is checked is that every
/// `required` property is present and that nothing arrived which the schema never declared. A
/// model that invented a property has not called the tool correctly, and that is the failure worth
/// catching — deep validation would be a library, and this needs a fact.
fn fits(given: &serde_json::Value, schema: &serde_json::Value) -> bool {
    let Some(given) = given.as_object() else {
        return false;
    };
    let properties = schema["properties"].as_object();
    if let Some(required) = schema["required"].as_array() {
        for key in required.iter().filter_map(|it| it.as_str()) {
            if !given.contains_key(key) {
                return false;
            }
        }
    }
    if let Some(properties) = properties {
        for key in given.keys() {
            if !properties.contains_key(key) {
                return false;
            }
        }
    }
    true
}

/// Two values that mean the same thing.
///
/// `12` and `12.0` and `"12"` are one number, and a benchmark that failed a model for sending a
/// string would be measuring a server's JSON habits. Strings compare case-insensitively and by
/// prefix, so `kilometres` matches `kilometers` and `km` does not — which is deliberate: an
/// abbreviation is a different answer to *what unit*.
fn same(got: &serde_json::Value, want: &serde_json::Value) -> bool {
    if let (Some(a), Some(b)) = (as_number(got), as_number(want)) {
        return (a - b).abs() < 1e-9;
    }
    match (got.as_str(), want.as_str()) {
        (Some(a), Some(b)) => {
            let a = a.trim().to_lowercase();
            let b = b.trim().to_lowercase();
            // `kilometres` and `kilometers`: the same word, two spellings, and neither is wrong.
            let stem = |raw: &str| raw.replace("re", "er");
            stem(&a) == stem(&b)
        }
        _ => got == want,
    }
}

fn as_number(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|raw| raw.trim().parse().ok()))
}

/// A tool call a model wrote into its answer instead of emitting.
fn written(text: &str) -> Option<(String, serde_json::Value)> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(&text[start..=end]).ok()?;
    // The two shapes seen in the wild: `{name, arguments}` and `{action, action_input}`.
    let name = value["name"]
        .as_str()
        .or_else(|| value["action"].as_str())
        .or_else(|| value["tool"].as_str())?;
    let arguments = if value["arguments"].is_null() {
        value["action_input"].clone()
    } else {
        value["arguments"].clone()
    };
    Some((name.to_owned(), arguments))
}

/// One request, with tools where the trial offers them.
fn ask(
    at: &str,
    model: &str,
    prompt: &str,
    tools: Option<&[serde_json::Value]>,
) -> Result<serde_json::Value, String> {
    ask_with(
        at,
        model,
        &[serde_json::json!({ "role": "user", "content": prompt })],
        tools,
    )
}

/// The same request, with a whole conversation rather than one message.
///
/// **One request shape, not two.** An agentic trial that asked differently from a single-call one
/// would be measuring the harness — the mistake this file already made once by declaring tools in
/// a test the product does not use.
fn ask_with(
    at: &str,
    model: &str,
    messages: &[serde_json::Value],
    tools: Option<&[serde_json::Value]>,
) -> Result<serde_json::Value, String> {
    let at = at
        .trim_end_matches('/')
        .replace("://localhost:", "://127.0.0.1:")
        .replace("://[::1]:", "://127.0.0.1:");
    let mut body = serde_json::json!({
        "model": model,
        "messages": messages,
        "max_tokens": crate::suite::TRIAL_TOKENS,
        "stream": false,
        // Fixed, so the same question asked of two models is the same question. A benchmark
        // where one model sampled at 0.8 compares two settings.
        "temperature": 0.0,
        "seed": 1,
    });
    if let Some(tools) = tools {
        body["tools"] = serde_json::Value::Array(tools.to_vec());
    }

    ureq::post(&format!("{at}/v1/chat/completions"))
        .send_json(body)
        .map_err(|why| match why {
            // The server's own words. It knows why far better than a sentence written here.
            ureq::Error::Status(code, said) => {
                format!("{code}: {}", said.into_string().unwrap_or_default().trim())
            }
            other => other.to_string(),
        })?
        .into_json()
        .map_err(|why| format!("answered something unreadable: {why}"))
}

/// The Standard phase: one model, the same way as every other.
///
/// **The speed reading first and the trials after.** A model that cannot answer at all fails the
/// first thing rather than eleven, and a run that is going to fail should say so in a minute
/// rather than in twenty.
///
/// **Two names, because a model has two.** `asked_as` is what this server calls it on the wire;
/// `known_as` is who it is on the deck. They are often different — Ollama's `gemma4:12b` is
/// llama.cpp's `gemma4-12b` — and the first real run filed a card under the wire name, so MODELS
/// could not find the result it had just taken. An id identifies and a wire name addresses; the
/// card is a fact about the model, so it is filed under the identity.
pub fn standard(
    at: &str,
    asked_as: &str,
    known_as: &str,
    conditions: Conditions,
    watching: &dyn Fn(&str, usize, usize),
) -> Card {
    let ask_for = Ask {
        context: conditions.context,
        ..Ask::default()
    };
    watching("speed", 0, 1);
    let speed = crate::bench::measure(at, asked_as, &ask_for, &|n, of| watching("speed", n, of));

    let every = crate::suite::all();
    let total = every.len();
    let mut answers = Vec::with_capacity(total);
    for (n, trial) in every.iter().enumerate() {
        watching(trial.id(), n + 1, total);
        answers.push(attempt(at, asked_as, trial));
    }

    Card {
        model: known_as.to_owned(),
        conditions,
        speed,
        answers,
    }
}

/// Everything measured in one sitting, with the models hidden for the reading half.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Table {
    pub cards: Vec<Card>,
    pub blind: Blind,
}

impl Table {
    pub fn of(cards: Vec<Card>) -> Self {
        let models: Vec<String> = cards.iter().map(|it| it.model.clone()).collect();
        Self {
            blind: Blind::of(&models),
            cards,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trials::{Expected, Tool};

    fn convert() -> Tool {
        Tool {
            name: "convert_units".to_owned(),
            description: String::new(),
            arguments: serde_json::json!({
                "type": "object",
                "properties": {
                    "value": { "type": "number" },
                    "from": { "type": "string" },
                    "to": { "type": "string" }
                },
                "required": ["value", "from", "to"]
            }),
        }
    }

    fn expected() -> Expected {
        Expected {
            tool: "convert_units".to_owned(),
            arguments: [
                ("value".to_owned(), serde_json::json!(12)),
                ("from".to_owned(), serde_json::json!("miles")),
                ("to".to_owned(), serde_json::json!("kilometres")),
            ]
            .into_iter()
            .collect(),
            answer: Some("19.31".to_owned()),
        }
    }

    fn answered_with(call: serde_json::Value) -> serde_json::Value {
        serde_json::json!({ "choices": [{ "message": { "tool_calls": [call] } }] })
    }

    #[test]
    fn a_correct_call_answers_all_five() {
        let said = answered_with(serde_json::json!({
            "function": {
                "name": "convert_units",
                "arguments": "{\"value\": 12, \"from\": \"miles\", \"to\": \"kilometres\"}"
            }
        }));
        let got = inspect(&said, "", &expected(), &[convert()]);
        assert!(got.called && got.right_tool && got.right_arguments && got.valid_shape && got.ran);
        assert_eq!(got.right_answer, Some(true), "12 miles is 19.31 km");
    }

    #[test]
    fn the_right_tool_with_the_wrong_numbers_is_not_a_pass() {
        // Which is the whole reason the five are kept apart: this call is well-formed, valid
        // against the schema, and answers the wrong question.
        let said = answered_with(serde_json::json!({
            "function": {
                "name": "convert_units",
                "arguments": "{\"value\": 5, \"from\": \"miles\", \"to\": \"kilometres\"}"
            }
        }));
        let got = inspect(&said, "", &expected(), &[convert()]);
        assert!(got.called && got.right_tool && got.valid_shape);
        assert!(!got.right_arguments, "5 is not 12");
        assert_eq!(got.right_answer, Some(false));
    }

    /// The wire shape a thinking model actually returned, measured on this machine.
    fn thought_and_never_answered() -> serde_json::Value {
        serde_json::json!({ "choices": [{
            "finish_reason": "length",
            "message": {
                "role": "assistant",
                "content": "",
                "reasoning_content": "*   Start day: Tuesday.
        *   Duration: 45 days"
            }
        }]})
    }

    fn card_of(answers: Vec<Answered>) -> Card {
        Card {
            model: "m".to_owned(),
            conditions: Conditions::default(),
            speed: crate::bench::Measured::default(),
            answers,
        }
    }

    fn answered(trial: &str, outcome: Outcome) -> Answered {
        Answered {
            trial: trial.to_owned(),
            model: "m".to_owned(),
            said: String::new(),
            outcome,
        }
    }

    #[test]
    fn a_column_is_three_facts_and_the_counts_they_came_from() {
        /*
            The owner's arithmetic, and it is the reason these are three: four of five reasoning
            trials answered, all four right.

                correctness 100%   (4/4 of what it said)
                completion   80%   (4/5 of what it was asked)
                effective    80%   (4/5 came back right)

            One figure can say only one of those. Collapsed, an empty answer either counts as
            wrong — it was never given — or costs nothing, which is worse: a task nobody completed
            did not go well.
        */
        let ids: Vec<String> = crate::suite::reasoning()
            .iter()
            .map(|it| it.id().to_owned())
            .collect();
        assert_eq!(ids.len(), 5, "the suite this test is written against");

        let mut answers: Vec<Answered> = ids
            .iter()
            .take(4)
            .map(|id| answered(id, Outcome::Reasoning { correct: true }))
            .collect();
        answers.push(answered(
            &ids[4],
            Outcome::Refused {
                why: "it ran out of room".to_owned(),
            },
        ));

        let column = card_of(answers).column(Kind::Reasoning);
        assert_eq!((column.correct, column.answered, column.asked), (4.0, 4, 5));
        assert_eq!(column.correctness, Some(1.0));
        assert_eq!(column.completion, Some(0.8));
        assert_eq!(column.effective, Some(0.8));
    }

    #[test]
    fn two_right_of_two_answered_out_of_three_asked() {
        // The owner's second example: coding correct twice out of two answers, three trials.
        // 100% correct, 66.7% completed, 66.7% effective.
        let ids: Vec<String> = crate::suite::coding()
            .iter()
            .map(|it| it.id().to_owned())
            .collect();
        assert_eq!(ids.len(), 3);

        let answers = vec![
            answered(
                &ids[0],
                Outcome::Coding {
                    passed: 5,
                    total: 5,
                    note: None,
                },
            ),
            answered(
                &ids[1],
                Outcome::Coding {
                    passed: 5,
                    total: 5,
                    note: None,
                },
            ),
            answered(
                &ids[2],
                Outcome::Refused {
                    why: "no answer".to_owned(),
                },
            ),
        ];
        let column = card_of(answers).column(Kind::Coding);
        assert_eq!(column.correctness, Some(1.0));
        assert!((column.completion.expect("asked") - 2.0 / 3.0).abs() < 1e-9);
        assert!((column.effective.expect("asked") - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!((column.correct, column.answered, column.asked), (2.0, 2, 3));
    }

    #[test]
    fn effective_is_derived_from_the_counts_and_not_from_the_other_two() {
        /*
            Three numbers that must agree, from one measurement. Multiplying two rounded rates
            would drift from the counts beside them — and the counts are the evidence.
        */
        let ids: Vec<String> = crate::suite::reasoning()
            .iter()
            .map(|it| it.id().to_owned())
            .collect();
        let answers = vec![
            answered(&ids[0], Outcome::Reasoning { correct: true }),
            answered(&ids[1], Outcome::Reasoning { correct: false }),
            answered(&ids[2], Outcome::Reasoning { correct: true }),
            answered(
                &ids[3],
                Outcome::Refused {
                    why: "x".to_owned(),
                },
            ),
            answered(
                &ids[4],
                Outcome::Refused {
                    why: "x".to_owned(),
                },
            ),
        ];
        let column = card_of(answers).column(Kind::Reasoning);
        let by_counts = column.correct / column.asked as f64;
        assert_eq!(column.effective, Some(by_counts));
        assert!((by_counts - 0.4).abs() < 1e-9, "2 right of 5 asked");
        assert!((column.correctness.expect("answered") - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn nothing_asked_is_not_a_zero_anywhere() {
        // A card from a suite this build no longer has. Every rate is unknown; none is nought.
        let column = card_of(Vec::new()).column(Kind::Tools);
        assert_eq!(column.asked, 0);
        assert_eq!(column.correctness, None);
        assert_eq!(column.completion, None);
        assert_eq!(column.effective, None);
    }

    #[test]
    fn an_answer_that_never_arrived_is_unanswered_and_not_wrong() {
        /*
            gemma4:12b spent every one of 700 tokens in `reasoning_content` and returned an empty
            `content` with `finish_reason: "length"`. The first version scored that as a wrong
            answer, which would have ranked every thinking model below every model that answers
            immediately — for a reason about the harness rather than about either of them.
        */
        let outcome = read_or_refuse(&thought_and_never_answered());
        match outcome {
            Some(Outcome::Refused { why }) => {
                assert!(why.contains("thinking"), "{why}");
                assert!(
                    why.contains(&crate::suite::TRIAL_TOKENS.to_string()),
                    "{why}"
                );
            }
            other => panic!("scored instead of refused: {other:?}"),
        }
    }

    #[test]
    fn a_tool_call_with_nothing_said_is_the_right_answer_not_an_empty_one() {
        // The one place an empty `content` is exactly right: the model reached for something
        // instead of talking, which is the thing the trial measures.
        let mut said = answered_with(serde_json::json!({
            "function": { "name": "convert_units", "arguments": "{}" }
        }));
        said["choices"][0]["finish_reason"] = serde_json::json!("length");
        said["choices"][0]["message"]["content"] = serde_json::json!("");
        assert!(read_or_refuse(&said).is_none(), "a call is an answer",);
    }

    #[test]
    fn a_card_is_filed_under_who_the_model_is_and_asked_for_by_what_the_wire_calls_it() {
        // Ollama's `gemma4:12b` is llama.cpp's `gemma4-12b`. The first real run filed the card
        // under the wire name, so MODELS could not find the result it had just taken.
        let card = Card {
            model: "gemma4:12b".to_owned(),
            conditions: Conditions::default(),
            speed: crate::bench::Measured::default(),
            answers: vec![Answered {
                trial: "r-days".to_owned(),
                model: "gemma4-12b".to_owned(),
                said: String::new(),
                outcome: Outcome::Refused {
                    why: "x".to_owned(),
                },
            }],
        };
        assert_ne!(card.model, card.answers[0].model, "two names, on purpose");
    }

    #[test]
    fn an_invented_property_does_not_fit_the_schema() {
        // A model that added a property the tool never declared has not called it correctly, and
        // that is the failure worth catching without writing a JSON Schema library.
        let said = answered_with(serde_json::json!({
            "function": {
                "name": "convert_units",
                "arguments": "{\"value\": 12, \"from\": \"miles\", \"to\": \"kilometres\", \"precision\": 2}"
            }
        }));
        let got = inspect(&said, "", &expected(), &[convert()]);
        assert!(got.right_arguments, "the three it needed are all there");
        assert!(!got.valid_shape, "and it invented a fourth");
        assert!(!got.ran);
    }

    #[test]
    fn a_missing_required_argument_does_not_fit_either() {
        let said = answered_with(serde_json::json!({
            "function": { "name": "convert_units", "arguments": "{\"value\": 12}" }
        }));
        let got = inspect(&said, "", &expected(), &[convert()]);
        assert!(got.called && got.right_tool);
        assert!(!got.valid_shape);
        assert!(!got.right_arguments);
    }

    #[test]
    fn a_number_sent_as_a_string_is_the_same_number() {
        // A benchmark that failed a model for a server's JSON habits would be measuring the
        // server.
        let said = answered_with(serde_json::json!({
            "function": {
                "name": "convert_units",
                "arguments": "{\"value\": \"12\", \"from\": \"Miles\", \"to\": \"kilometers\"}"
            }
        }));
        let got = inspect(&said, "", &expected(), &[convert()]);
        assert!(
            got.right_arguments,
            "12 is 12, and kilometers is kilometres"
        );
        assert_eq!(got.right_answer, Some(true));
    }

    #[test]
    fn a_call_written_into_the_answer_counts_and_says_so() {
        /*
            `CLAUDE.md` already holds this rule for turns: a model whose chat template does not
            emit `tool_calls` writes the JSON instead, and Epoch reads it. A benchmark that ignored
            those would report one model as toolless on Ollama and capable on llama.cpp.

            It is recorded as what it is, because the difference decides whether every backend can
            use it.
        */
        let said = serde_json::json!({ "choices": [{ "message": { "content": "x" } }] });
        let text =
            r#"{"name":"convert_units","arguments":{"value":12,"from":"miles","to":"kilometres"}}"#;
        let got = inspect(&said, text, &expected(), &[convert()]);
        assert!(got.called && got.right_tool && got.right_arguments && got.valid_shape);
        assert!(
            got.note.as_deref().is_some_and(|it| it.contains("written")),
            "{got:?}"
        );
    }

    #[test]
    fn no_call_at_all_is_five_noes_and_not_a_crash() {
        let said =
            serde_json::json!({ "choices": [{ "message": { "content": "It is about 19 km." } }] });
        let got = inspect(&said, "It is about 19 km.", &expected(), &[convert()]);
        assert!(!got.called);
        assert_eq!(Outcome::Tools(got).score(), Some(0.0));
    }

    #[test]
    fn a_column_with_nothing_in_it_has_no_score() {
        // A model whose coding trials could not run because the machine has no interpreter is not
        // a model that failed them, and a zero in that column would say it was.
        let card = Card {
            model: "m".to_owned(),
            conditions: Conditions::default(),
            speed: Measured::default(),
            answers: vec![Answered {
                trial: "c-1".to_owned(),
                model: "m".to_owned(),
                said: String::new(),
                outcome: Outcome::Refused {
                    why: "no python".to_owned(),
                },
            }],
        };
        assert_eq!(card.score(Kind::Coding), None);
        assert_eq!(card.tests(), (0, 0));
    }

    #[test]
    fn a_table_labels_the_models_in_the_order_they_were_run() {
        let table = Table::of(vec![
            Card {
                model: "first".to_owned(),
                conditions: Conditions::default(),
                speed: Measured::default(),
                answers: Vec::new(),
            },
            Card {
                model: "second".to_owned(),
                conditions: Conditions::default(),
                speed: Measured::default(),
                answers: Vec::new(),
            },
        ]);
        assert_eq!(table.blind.letter_of("first"), Some("A"));
        assert_eq!(table.blind.letter_of("second"), Some("B"));
    }
}
