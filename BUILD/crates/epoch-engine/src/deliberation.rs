//! Which rungs of the reasoning ladder a given brain can actually honour.
//!
//! ## Why the ladder is canonical and the *scale* is not
//!
//! [`Reasoning`] is character identity: a Guardian deliberates more than a Researcher, on any
//! engine (ADR-0026). So the ladder lives in the Kernel and the value travels with the character
//! into every World.
//!
//! What differs is **granularity**. One runtime has a boolean. A hosted API has four effort
//! levels. Claude Code's CLI has five and includes `xhigh`, which nothing else does. A single
//! hardcoded list of buttons would therefore be wrong for every backend except the one it was
//! written against — and worse, it would be wrong *silently*, offering a rung that quietly
//! became another one.
//!
//! This is ADR-0026's rule applied to one more parameter: **the brain declares the surface; the
//! character holds the value.** The UI renders whatever comes back and knows nothing about
//! anybody's ladder.
//!
//! ## Measured where it can be, honest where it cannot
//!
//! Claude Code's rungs are read from the program's own help — `--effort <level>` (low, medium,
//! high, xhigh, max) — not remembered. That is also why `Off` is absent from its scale: the CLI
//! has no such level, and a slider that offered it would be an instrument that lies.
//!
//! A brain we have no measurement for gets **no scale**, and the surface shows no shortcut.
//! Nothing here guesses a ladder on a backend's behalf.

use epoch_kernel::{Brain, Reasoning};

/// The rungs a **model** can be asked for.
///
/// `XHigh` is deliberately absent: no model backend in this build distinguishes it, so offering
/// it would put a rung on the slider that silently resolves to `High`. A control that does not
/// change anything is worse than a missing one — it teaches people the instruments are
/// decorative (the Launcher's rule).
const FOR_A_MODEL: [Reasoning; 5] = [
    Reasoning::Off,
    Reasoning::Low,
    Reasoning::Medium,
    Reasoning::High,
    Reasoning::Max,
];

/// The rungs an **agent** takes.
///
/// No `Off`, on either of them: neither program has a level that means "do not think", so the
/// scale starts where they start.
///
/// The same five for both, and that was measured rather than assumed — `low`, `medium`, `high`,
/// `xhigh`, `max` each ran against Claude Code's `--effort` and Codex's
/// `model_reasoning_effort`. Two independent programs sharing `xhigh` is also what settles the
/// doubt about adding that rung to the Kernel: it is a rung of the concept, not one vendor's word.
const FOR_AN_AGENT: [Reasoning; 5] = [
    Reasoning::Low,
    Reasoning::Medium,
    Reasoning::High,
    Reasoning::XHigh,
    Reasoning::Max,
];

/// What this brain can be asked for, in order, weakest first.
///
/// Empty means *no shortcut* — an agent whose ladder nobody has measured. Returning a plausible
/// default instead would be the same invention the cold-instrument rule exists to prevent.
pub fn scale(brain: &Brain) -> &'static [Reasoning] {
    match brain {
        Brain::Model { .. } => &FOR_A_MODEL,
        // The model matters, not only the agent: Claude Code's effort levels exist for most of
        // its models and not for Haiku, and offering a rung that changes nothing would teach
        // somebody the instrument is decorative.
        Brain::Agent { agent, model } if agent == crate::agents::claude::ID => {
            // Haiku has no effort levels at all — its own picker says so, and the command line
            // accepts the flag and ignores it. A control that changes nothing is worse than a
            // missing one.
            if crate::agents::claude::supports_effort(model) {
                &FOR_AN_AGENT
            } else {
                &[]
            }
        }
        Brain::Agent { agent, .. } if agent == crate::agents::codex::ID => &FOR_AN_AGENT,
        // An agent nobody has measured. A guessed ladder is the invention the cold-instrument
        // rule exists to prevent.
        Brain::Agent { .. } => &[],
    }
}

/// The rungs a model may be offered, for a surface that is editing a model's parameters.
///
/// Exposed so the character form asks the same question the shortcut does. Two places listing
/// the ladder independently is how one of them ends up offering a rung the other refuses.
pub fn for_models() -> &'static [Reasoning] {
    &FOR_A_MODEL
}

/// One rung, as a surface receives it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rung {
    /// What the Engine accepts back. A surface may never invent one.
    pub id: &'static str,
    /// What to call it on screen — from the Kernel, so two surfaces cannot disagree.
    pub label: &'static str,
}

/// The whole shortcut, for one character.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dial {
    /// Weakest first. Empty means this brain has no measured ladder, so show no shortcut.
    pub rungs: Vec<Rung>,
    /// Where they are now. `None` is a real position — *the brain's own default* — and it sits
    /// at the left of the scale before the first rung, because "nothing was asked for" is not
    /// the same as "the least".
    pub chosen: Option<&'static str>,
    /// Whose ladder this is, so the surface can say it without knowing anything about brains.
    pub brain: String,
    /// Which agent, when it is one. `None` for a model.
    ///
    /// The id rather than the name: a surface asks the Engine what a mode means for *this* agent,
    /// and a name is a label a pack could one day change.
    pub agent_id: Option<String>,
    /// Whether that brain is an agent.
    ///
    /// Here because a surface must not infer it from the name — "Claude Code" is a string a
    /// model could one day be called. It exists for one honest sentence: an agent enforces
    /// Epoch's autonomy mode *itself*, and `Manual` means something different there.
    pub agent: bool,
}

/// Read the dial for one character.
pub fn dial(definition: &epoch_kernel::CharacterDefinition) -> Dial {
    let Some(mind) = definition.mind.as_ref() else {
        return Dial {
            rungs: Vec::new(),
            chosen: None,
            brain: String::new(),
            agent: false,
            agent_id: None,
        };
    };
    Dial {
        rungs: scale(&mind.brain)
            .iter()
            .map(|r| Rung {
                id: r.as_str(),
                label: r.label(),
            })
            .collect(),
        chosen: mind.parameters.reasoning.map(Reasoning::as_str),
        agent: matches!(mind.brain, Brain::Agent { .. }),
        agent_id: match &mind.brain {
            Brain::Agent { agent, .. } => Some(agent.clone()),
            Brain::Model { .. } => None,
        },
        brain: match &mind.brain {
            Brain::Model { model, .. } => model.clone(),
            Brain::Agent { agent, .. } => crate::agents::name_of(agent)
                .unwrap_or(agent.as_str())
                .to_owned(),
        },
    }
}

/// Move one character up or down their ladder, and write it.
///
/// A **narrow** setter rather than a trip through the whole character form. The form validates
/// every field and rewrites the file from all of them, which is right when somebody is editing a
/// character and wrong for a shortcut used mid-conversation: it would make a one-rung change
/// depend on a name, a role and an archetype being valid at that moment.
///
/// `None` returns them to the brain's own default. Refused rather than clamped when the rung is
/// not on *their* scale — a surface that could set `Off` on an agent with no such level would be
/// storing an instruction nobody can carry out.
pub fn choose(
    registry: &mut crate::definition::DefinitionRegistry,
    id: &epoch_kernel::CharacterId,
    rung: Option<&str>,
) -> Result<(), String> {
    let mut definition = registry
        .character(id)
        .ok_or_else(|| format!("no character called '{id}'"))?
        .clone();
    let mind = definition
        .mind
        .as_mut()
        .ok_or_else(|| format!("{} has no brain assigned", definition.name))?;

    let wanted = match rung {
        None => None,
        Some(raw) => {
            let rung = Reasoning::from_id(raw)
                .ok_or_else(|| format!("'{raw}' is not a level this build knows"))?;
            if !scale(&mind.brain).contains(&rung) {
                return Err(format!(
                    "{} cannot be asked for {} deliberation",
                    definition.name,
                    rung.label().to_lowercase()
                ));
            }
            Some(rung)
        }
    };

    mind.parameters.reasoning = wanted;
    registry.save(definition).map_err(|why| why.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Brain {
        Brain::Model {
            provider: "ollama".into(),
            model: "qwen3:14b".into(),
        }
    }

    fn claude() -> Brain {
        Brain::Agent {
            agent: crate::agents::claude::ID.into(),
            model: String::new(),
        }
    }

    fn claude_on(model: &str) -> Brain {
        Brain::Agent {
            agent: crate::agents::claude::ID.into(),
            model: model.into(),
        }
    }

    /// A model with no effort levels gets no dial.
    ///
    /// The CLI says so itself — *"Effort not supported for Haiku"* — and, tested, it accepts
    /// `--effort max --model haiku` without a word and ignores it. A slider that silently does
    /// nothing is worse than a missing one.
    #[test]
    fn a_model_without_effort_levels_is_offered_none() {
        assert!(scale(&claude_on("haiku")).is_empty());
        assert!(scale(&claude_on("claude-haiku-4-5")).is_empty());
        assert!(!scale(&claude_on("opus")).is_empty());
        // Nothing named means the agent's own choice, which may well have one.
        assert!(!scale(&claude_on("")).is_empty());
    }

    #[test]
    fn a_scale_only_offers_rungs_that_backend_can_tell_apart() {
        // `xhigh` exists on Claude Code's command line and nowhere else in this build. Offering
        // it for a model would be a slider position that changes nothing.
        assert!(!scale(&model()).contains(&Reasoning::XHigh));
        assert!(scale(&claude()).contains(&Reasoning::XHigh));
    }

    #[test]
    fn claude_code_has_no_off_because_the_program_has_none() {
        // `--effort` takes low|medium|high|xhigh|max. There is no "do not think".
        assert!(!scale(&claude()).contains(&Reasoning::Off));
        assert!(scale(&model()).contains(&Reasoning::Off));
    }

    #[test]
    fn a_scale_runs_weakest_first() {
        // The slider's left is less deliberation on every brain, or the control means a
        // different thing depending on who is selected.
        for brain in [model(), claude()] {
            let rungs = scale(&brain);
            assert!(rungs.windows(2).all(|pair| pair[0] < pair[1]), "{rungs:?}");
        }
    }

    #[test]
    fn an_agent_nobody_has_measured_gets_no_scale() {
        // Not a guessed ladder. A surface showing no shortcut is honest; one showing five
        // invented rungs is an instrument that lies.
        // Was `codex`, until Codex was measured and this test failed — which is the test doing
        // its job: which agents have a known ladder is a fact, not an assumption.
        let unknown = Brain::Agent {
            agent: "some-agent-nobody-has-run".into(),
            model: String::new(),
        };
        assert!(scale(&unknown).is_empty());
    }
}
