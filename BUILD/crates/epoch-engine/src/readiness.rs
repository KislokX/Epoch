//! What this machine has, what it does not, and what to do about each.
//!
//! ## Look first; teach only where looking found nothing
//!
//! This file's first draft said *detecting is guessing*, and that was wrong in a way that closed
//! doors. **Detecting is looking, and looking is measuring.** Guessing is concluding without
//! having looked. Putting the two under one word made a legitimate search sound like an
//! invention, and the correction is the owner's: search, and *then* teach if the search came back
//! empty.
//!
//! So the order is look, look wider, and only then explain:
//!
//! - **Ask the program.** `claude --version`, `/api/tags`. A thing runs or it does not.
//! - **Read the environment.** A `DEEPSEEK_API_KEY` sitting in the user's own shell is a fact
//!   about this machine, not a hypothesis — and it is how those CLIs are configured anyway. Only
//!   *that it exists*: [`keys_in_the_environment`] never reads a value, which is the same
//!   guarantee `ConnectionNote` gives by having nowhere to put one.
//! - **Then, and only then, say what would be needed.** An address on somebody's network is
//!   genuinely not here to be found.
//!
//! The one search Epoch still will not do is sweep a network for servers. Not because it would
//! guess — it would measure — but because it means sending traffic to machines that are not the
//! user's business to probe, slowly, in a pattern that reads to everything else on that network
//! as a port scan. That is a cost the user did not ask for. **A single address they typed costs
//! nothing and is exact.**
//!
//! ## Why this is a projection and not a wizard
//!
//! A wizard is a sequence somebody has to finish. This is a *reading*: every row is true right
//! now, in any order, and a person who has three of six things set up is not halfway through
//! anything — they have three things. Epoch already refuses to invent progress in the World, and
//! a setup screen is not the place to start.
//!
//! ## What is deliberately absent
//!
//! Installation instructions. "Run `brew install …`" is documentation that ages in somebody
//! else's repository, and a stale command in a setup screen is worse than none — it is confidently
//! wrong. What a row carries is what Epoch measured and, where Epoch can act, an action it
//! genuinely performs (`Do::SignIn` opens the agent's own login; `Do::Configure` opens the deck).

use crate::agent::AgentStatus;
use crate::provider::ProviderStatus;

/// Whether this thing is usable, and if not, why not.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    /// Measured and working.
    Ready,
    /// Measured and not here. Never a guess — something was asked and answered.
    Missing,
    /// Here, and needing something from the user before it can work.
    NeedsYou,
    /// Nothing to detect: this exists only once somebody adds it.
    Unset,
}

/// What Epoch can do about a row, if anything.
///
/// Deliberately small, and everything in it is something Epoch actually performs. A variant
/// meaning "tell the user to go and install it" would be a button that types a sentence.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Do {
    /// Nothing to offer — the fix is outside Epoch.
    Nothing,
    /// Start the program's own sign-in, in its own window. Epoch never sees the password
    /// (`CLAUDE.md`: it opens the door and never holds the key).
    SignIn,
    /// Open the deck where this is entered.
    Configure,
}

/// One thing this machine either has or does not.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    /// Stable id — the agent's or backend's, so an action knows what it is acting on.
    pub id: String,
    pub name: String,
    pub state: State,
    /// One sentence, in the user's terms. **Never invented**: it is what the thing itself said,
    /// or what Epoch would need from them.
    pub note: String,
    pub act: Do,
}

/// Read this machine, as a list somebody can act on.
///
/// Pure over what the surveys returned, so the whole projection is testable without a network,
/// an install, or a key.
pub fn survey(agents: &[AgentStatus], providers: &[ProviderStatus]) -> Vec<Row> {
    let mut rows: Vec<Row> = agents
        .iter()
        .map(|agent| {
            let (state, act) = match (agent.installed, agent.signed_in) {
                // Signed in, or a program that has no such question. Working either way.
                (true, Some(true) | None) => (State::Ready, Do::Nothing),
                // Installed and signed out. Two different fixes, which is why these are two
                // fields — a surface that collapsed them would send somebody to reinstall a
                // program that is already there.
                (true, Some(false)) => (State::NeedsYou, Do::SignIn),
                (false, _) => (State::Missing, Do::Nothing),
            };
            Row {
                id: agent.id.clone(),
                name: agent.name.clone(),
                state,
                note: agent.note.clone().unwrap_or_else(|| match &agent.account {
                    // What it said about itself, when it said something.
                    Some(who) => format!("Signed in as {who}."),
                    None => "Installed and ready.".into(),
                }),
                act,
            }
        })
        .collect();

    rows.extend(providers.iter().map(|backend| Row {
        id: backend.id.clone(),
        name: backend.name.clone(),
        state: if backend.online {
            State::Ready
        } else {
            State::NeedsYou
        },
        note: backend.note.clone().unwrap_or_else(|| {
            // Measured, and worth saying: "online" alone does not tell somebody whether there is
            // anything to run on it.
            match backend.models.len() {
                0 => "Answering, with no models installed yet.".into(),
                1 => "Answering, with 1 model.".into(),
                n => format!("Answering, with {n} models."),
            }
        }),
        act: if backend.online {
            Do::Nothing
        } else {
            Do::Configure
        },
    }));

    rows
}

/// Hosted backends whose key is already sitting in this machine's environment.
///
/// **The search the first draft refused to make.** These variables are how the vendors' own
/// tools are configured, so a person who has ever used one has the key here already — and asking
/// them to find it again, from a screen that could have looked, is Epoch making somebody prove
/// something it could see.
///
/// **Existence only, never the value.** The return type has nowhere to put a secret, which is the
/// same guarantee `ConnectionNote` gives and the same reason: a promise the compiler keeps beats
/// a rule somebody has to remember. Epoch does not copy it either — a key in an environment
/// belongs to the environment, and importing it would make two places to revoke.
///
/// `asked` is the lookup, so this is testable without touching the real environment.
pub fn keys_in_the_environment(asked: &dyn Fn(&str) -> bool) -> Vec<String> {
    [
        ("DEEPSEEK_API_KEY", "Deepseek"),
        ("GEMINI_API_KEY", "Gemini"),
        ("GOOGLE_API_KEY", "Gemini"),
        ("ZHIPUAI_API_KEY", "GLM"),
        ("GROQ_API_KEY", "Groq"),
        ("MISTRAL_API_KEY", "Mistral"),
        ("OPENAI_API_KEY", "OpenAI"),
        ("ANTHROPIC_API_KEY", "Anthropic"),
    ]
    .into_iter()
    .filter(|(variable, _)| asked(variable))
    .map(|(_, vendor)| vendor.to_owned())
    // Two variables can name one vendor (`GEMINI_API_KEY`, `GOOGLE_API_KEY`), and saying it twice
    // would read as two accounts.
    .fold(Vec::new(), |mut found, vendor| {
        if !found.contains(&vendor) {
            found.push(vendor);
        }
        found
    })
}

/// The things that only exist once somebody adds them.
///
/// Constant, because there is nothing to measure: an address and a key are facts about the user's
/// arrangements, not about this machine. They are listed anyway — a setup screen that showed only
/// what was found could not tell anybody that a remote machine is *possible*, which is the one
/// thing a person who has one needs to learn.
pub fn addable() -> Vec<Row> {
    [
        (
            "openai-compatible",
            "A server of your own",
            "llama.cpp, LM Studio, vLLM — anything speaking the OpenAI API. Needs its address.",
        ),
        (
            "hosted-key",
            "A hosted model",
            "Deepseek, GLM, Gemini and the rest. Needs a key from their own site; Epoch stores it \
             encrypted and never shows it again.",
        ),
        // Replaced below when the environment already holds one — see `addable_after_looking`.
        (
            "remote",
            "Another machine",
            "A backend on your network — the desktop under the desk, a Mac mini. Needs its \
             address; Epoch cannot go looking for one.",
        ),
    ]
    .into_iter()
    .map(|(id, name, note)| Row {
        id: id.into(),
        name: name.into(),
        state: State::Unset,
        note: note.into(),
        act: Do::Configure,
    })
    .collect()
}

/// [`addable`], after looking.
///
/// The whole shape of the owner's correction in one function: search first, and teach only what
/// the search did not answer. A row that says *"needs a key from their own site"* to somebody who
/// has had that key in their shell for a year is Epoch failing to look, and then explaining.
///
/// The key is still not taken. What changes is the sentence and the state: found, and one
/// confirmation away — rather than absent and needing a trip to a website.
pub fn addable_after_looking(asked: &dyn Fn(&str) -> bool) -> Vec<Row> {
    let found = keys_in_the_environment(asked);
    addable()
        .into_iter()
        .map(|row| {
            if row.id != "hosted-key" || found.is_empty() {
                return row;
            }
            Row {
                state: State::NeedsYou,
                note: format!(
                    "A key for {} is already in this machine's environment. Epoch has not read \
                     it — say the word and it will use it.",
                    found.join(", ")
                ),
                ..row
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(
        id: &str,
        installed: bool,
        signed_in: Option<bool>,
        note: Option<&str>,
    ) -> AgentStatus {
        AgentStatus {
            kind: crate::agents::kind_of(id).to_owned(),
            id: id.into(),
            name: id.into(),
            installed,
            version: None,
            looked_in: None,
            note: note.map(str::to_owned),
            signed_in,
            account: None,
            method: None,
        }
    }

    fn backend(id: &str, online: bool, models: &[&str], note: Option<&str>) -> ProviderStatus {
        ProviderStatus {
            id: id.into(),
            name: id.into(),
            machine: None,
            endpoint: "http://127.0.0.1:11434".into(),
            online,
            local: true,
            models: models.iter().map(|m| (*m).to_owned()).collect(),
            note: note.map(str::to_owned),
        }
    }

    #[test]
    fn installed_and_signed_out_is_not_the_same_row_as_not_installed() {
        // The distinction that made `signed_in` its own field. A machine with the desktop app
        // open answered "Not logged in · Please run /login", and a surface that could not tell
        // the two apart would send somebody to install a program they already have.
        let rows = survey(
            &[
                agent("claude-code", true, Some(false), Some("Not logged in.")),
                agent("codex", false, None, Some("not installed")),
            ],
            &[],
        );

        assert_eq!(rows[0].state, State::NeedsYou);
        assert_eq!(rows[0].act, Do::SignIn, "Epoch can open its own login");
        assert_eq!(rows[1].state, State::Missing);
        assert_eq!(
            rows[1].act,
            Do::Nothing,
            "installing is outside Epoch, and a button that says so is a button that lies"
        );
    }

    #[test]
    fn a_backend_that_answered_says_what_it_has() {
        // "Online" alone does not tell somebody whether there is anything to run.
        let rows = survey(
            &[],
            &[backend("ollama", true, &["gemma4:12b", "qwen3:14b"], None)],
        );

        assert_eq!(rows[0].state, State::Ready);
        assert!(rows[0].note.contains("2 models"), "{}", rows[0].note);
    }

    #[test]
    fn an_empty_backend_is_ready_and_says_it_is_empty() {
        // Ready and useless are different things, and only one of them is a problem to fix.
        let rows = survey(&[], &[backend("ollama", true, &[], None)]);

        assert_eq!(rows[0].state, State::Ready);
        assert!(rows[0].note.contains("no models"), "{}", rows[0].note);
    }

    #[test]
    fn a_key_already_in_the_environment_is_found_rather_than_asked_for() {
        // **The owner's correction, as a test.** The first draft called this searching
        // "guessing" and refused it, so a person with a key in their shell for a year would have
        // been sent to a website to fetch it again — Epoch failing to look, and then explaining.
        let shell = |name: &str| name == "DEEPSEEK_API_KEY";
        let rows = addable_after_looking(&shell);
        let hosted = rows.iter().find(|r| r.id == "hosted-key").expect("listed");

        assert_eq!(hosted.state, State::NeedsYou, "found, not absent");
        assert!(hosted.note.contains("Deepseek"), "{}", hosted.note);
        // Seen, never taken. A key in an environment belongs to the environment; copying it
        // would make two places to revoke.
        assert!(hosted.note.contains("has not read it"), "{}", hosted.note);
    }

    #[test]
    fn a_vendor_with_two_variable_names_is_still_one_vendor() {
        // `GEMINI_API_KEY` and `GOOGLE_API_KEY` both configure the same thing, and naming it
        // twice would read as two accounts.
        let both = |name: &str| name == "GEMINI_API_KEY" || name == "GOOGLE_API_KEY";
        assert_eq!(keys_in_the_environment(&both), vec!["Gemini".to_string()]);
    }

    #[test]
    fn an_empty_environment_still_gets_told_what_would_be_needed() {
        // Teaching is what happens *after* looking finds nothing — which is the half of the rule
        // that was right all along.
        let nothing = |_: &str| false;
        let rows = addable_after_looking(&nothing);
        let hosted = rows.iter().find(|r| r.id == "hosted-key").expect("listed");

        assert_eq!(hosted.state, State::Unset);
        assert!(hosted.note.contains("Needs a key"), "{}", hosted.note);
    }

    #[test]
    fn what_cannot_be_detected_is_listed_anyway() {
        // A screen showing only what was found could not tell anybody that a machine on their
        // network is possible — which is the one thing somebody with one needs to learn.
        let rows = addable();

        assert!(rows.iter().all(|row| row.state == State::Unset));
        assert!(rows.iter().all(|row| row.act == Do::Configure));
        // And each says what it needs, rather than how to install anything: a command in a setup
        // screen is documentation that ages in somebody else's repository.
        assert!(rows.iter().all(|row| row.note.contains("Needs")));
    }
}
