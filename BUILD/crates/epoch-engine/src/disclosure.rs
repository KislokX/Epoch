//! Where a turn goes, and whose decision that is (ADR-0029 §6, ADR-0025).
//!
//! ## What actually travels
//!
//! A composed turn is not a question. It carries the Chronicle, whatever the Project Root
//! contributed, the World's knowledge, and the crew's own words — everything the Composer put in
//! it (ADR-0012). Whether that is fine depends entirely on **where it is going**, and only the
//! user can answer that.
//!
//! ## Three destinations, three different decisions
//!
//! | destination | what it means |
//! |---|---|
//! | [`Where::Here`] | nothing leaves the machine |
//! | [`Where::Yours`] | it crosses the user's own network, to a machine they paired |
//! | [`Where::Elsewhere`] | it reaches a third party, under that party's terms |
//!
//! ADR-0025 already named the third for hosted providers. The second is new and it is genuinely
//! different: a Mac mini under the desk is not a vendor, and treating them the same would either
//! nag about something harmless or hide something that is not.
//!
//! ## Asked once, per destination, and remembered
//!
//! **Not per turn.** A question that appears every time is a question people click through, and
//! a click-through is not consent. It is asked the first time a turn would leave, the answer is
//! kept, and the Launcher shows what was answered — which is also where it is taken back.
//!
//! ## Derived, never stored twice
//!
//! Where a turn goes is a fact about the Service that will run it — `ProviderStatus::local`, and
//! whether that Service is a paired machine. Storing the destination beside a character would be
//! a second copy that goes stale the moment somebody reassigns them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Where a composed turn would go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Where {
    /// This machine. Nothing leaves.
    Here,
    /// A machine the user owns and paired.
    Yours,
    /// Somebody else's service.
    Elsewhere,
}

impl Where {
    pub const fn id(self) -> &'static str {
        match self {
            Where::Here => "here",
            Where::Yours => "yours",
            Where::Elsewhere => "elsewhere",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "here" => Some(Where::Here),
            "yours" => Some(Where::Yours),
            "elsewhere" => Some(Where::Elsewhere),
            _ => None,
        }
    }

    /// Whether anything leaves this machine at all.
    ///
    /// [`Where::Here`] needs no decision, and asking about it would train somebody to dismiss
    /// the question before it matters.
    pub const fn leaves(self) -> bool {
        !matches!(self, Where::Here)
    }

    /// The sentence a person is answering.
    pub const fn asks(self) -> &'static str {
        match self {
            Where::Here => "Nothing leaves this machine.",
            Where::Yours => {
                "This turn carries your conversation, your project's context and this World's \
                 knowledge across your own network, to a machine you paired. It does not reach \
                 anybody else."
            }
            Where::Elsewhere => {
                "This turn carries your conversation, your project's context and this World's \
                 knowledge to a service somebody else runs, under their terms."
            }
        }
    }
}

/// What the user has answered, per destination.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Disclosures {
    #[serde(default)]
    answered: BTreeMap<String, bool>,
}

impl Disclosures {
    pub fn load(vault: &Path) -> Self {
        std::fs::read_to_string(path(vault))
            .ok()
            .and_then(|raw| toml::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, vault: &Path) -> Result<(), String> {
        let body = toml::to_string_pretty(self).map_err(|err| err.to_string())?;
        std::fs::create_dir_all(vault).map_err(|err| err.to_string())?;
        std::fs::write(path(vault), body).map_err(|err| err.to_string())
    }

    /// Whether a turn may go there.
    ///
    /// `None` means **not asked yet** — which is the state a surface has to be able to see, so
    /// it can ask rather than assume either answer. Somewhere that never leaves the machine is
    /// always allowed and is never asked about.
    pub fn allowed(&self, going: Where) -> Option<bool> {
        if !going.leaves() {
            return Some(true);
        }
        self.answered.get(going.id()).copied()
    }

    /// Record what they answered. A refusal is kept exactly like a permission: it is an answer,
    /// and re-asking it would be ignoring them.
    pub fn answer(&mut self, going: Where, allowed: bool) {
        if going.leaves() {
            self.answered.insert(going.id().to_owned(), allowed);
        }
    }

    /// Forget one answer, so it is asked again. What a Launcher's "ask me again" does.
    pub fn forget(&mut self, going: Where) {
        self.answered.remove(going.id());
    }
}

/// Where a turn on this Service, with this model, would go.
///
/// Derived from what is already measured: a Provider that runs on this machine is [`Where::Here`]
/// unless it is one of the paired machines, and anything else is somebody else's.
///
/// ## The model, not only the Service
///
/// **A cloud model breaks the assumption this function was built on.** Ollama runs on the user's
/// own machine and its Provider is local — and `ollama pull gpt-oss:120b-cloud` gives it a model
/// it executes *on Ollama's servers*. Measured: that tag resolves in the registry like any
/// other, so the local daemon serves it and nothing about the Service says the turn left.
///
/// Answering `Here` for one would be Epoch saying *nothing leaves this machine* about a request
/// to a third party — the exact sentence [`Where::Here`] means, about the exact thing it is
/// supposed to rule out. So the destination is a fact about the pair, and a cloud model is
/// `Elsewhere` however local the program running it is.
pub fn destination(local: bool, paired_addresses: &[String], endpoint: &str, model: &str) -> Where {
    // First, because it overrules both of the others: a cloud model on a paired Mac still ends
    // up at Ollama, not at the Mac.
    if epoch_models::ollama_library::runs_in_the_cloud(model) {
        return Where::Elsewhere;
    }
    if paired_addresses.iter().any(|a| same_host(a, endpoint)) {
        return Where::Yours;
    }
    if local {
        return Where::Here;
    }
    Where::Elsewhere
}

/// Whether two addresses name the same machine, as far as a person means it.
///
/// Compared by host and port and nothing else: `http://10.0.0.4:11434` and
/// `http://10.0.0.4:11434/v1` are the same machine, and a user who typed one and configured the
/// other has not paired something different.
fn same_host(a: &str, b: &str) -> bool {
    fn authority(raw: &str) -> String {
        raw.trim()
            .trim_end_matches('/')
            .rsplit("://")
            .next()
            .unwrap_or_default()
            .split('/')
            .next()
            .unwrap_or_default()
            .to_lowercase()
    }
    !a.trim().is_empty() && authority(a) == authority(b)
}

fn path(vault: &Path) -> PathBuf {
    vault.join("disclosure.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_leaving_is_never_asked_about() {
        // A question that appears when there is nothing to decide is a question people learn to
        // dismiss — and then dismiss the one that mattered.
        let none = Disclosures::default();
        assert_eq!(none.allowed(Where::Here), Some(true));
        assert_eq!(none.allowed(Where::Yours), None);
        assert_eq!(none.allowed(Where::Elsewhere), None);
    }

    #[test]
    fn unasked_is_not_a_no_and_not_a_yes() {
        // The state a surface must be able to see. Defaulting either way would be Epoch
        // answering a question it says only the user can answer.
        let mut kept = Disclosures::default();
        assert_eq!(kept.allowed(Where::Elsewhere), None);

        kept.answer(Where::Elsewhere, false);
        assert_eq!(
            kept.allowed(Where::Elsewhere),
            Some(false),
            "a refusal is an answer, and re-asking it would be ignoring them"
        );

        kept.forget(Where::Elsewhere);
        assert_eq!(kept.allowed(Where::Elsewhere), None);
    }

    #[test]
    fn a_machine_you_paired_is_not_a_third_party() {
        // The distinction the whole type exists for: a Mac mini under the desk is not a vendor,
        // and treating them the same either nags about something harmless or hides something
        // that is not.
        let paired = vec!["http://10.0.0.4:11434".to_string()];

        assert_eq!(
            destination(false, &paired, "http://10.0.0.4:11434/v1", "qwen3:14b"),
            Where::Yours,
            "same host and port, whatever the path"
        );
        assert_eq!(
            destination(true, &paired, "http://localhost:11434", "qwen3:14b"),
            Where::Here
        );
        assert_eq!(
            destination(false, &paired, "https://api.anthropic.com", "qwen3:14b"),
            Where::Elsewhere
        );
    }

    #[test]
    fn a_remote_machine_nobody_paired_is_still_somebody_elses() {
        // Pointing at an address is not the same as pairing with it, and this is the case where
        // being generous would be wrong: an unpaired host on a LAN has no bond and no secret.
        assert_eq!(
            destination(false, &[], "http://10.0.0.9:11434", "qwen3:14b"),
            Where::Elsewhere
        );
    }

    #[test]
    fn what_is_shown_says_what_actually_travels() {
        // Not "data may be shared". The Chronicle, the project's context and the World's
        // knowledge — because that is what the Composer put in the turn.
        assert!(Where::Elsewhere.asks().contains("conversation"));
        assert!(Where::Elsewhere.asks().contains("project"));
        assert!(Where::Yours.asks().contains("does not reach anybody else"));
    }

    #[test]
    fn a_cloud_model_leaves_the_machine_however_local_the_program_running_it_is() {
        // **The assumption this function was built on, broken by a real thing.** Ollama runs on
        // the user's own computer and its Provider is local — and `ollama pull
        // gpt-oss:120b-cloud` gives it a model it executes on Ollama's servers. Measured: that
        // tag resolves in the registry like any other, so the local daemon serves it and nothing
        // about the *Service* says the turn left.
        //
        // Answering `Here` would be Epoch saying "nothing leaves this machine" about a request
        // to a third party, which is the exact sentence that variant means.
        let paired = vec!["http://10.0.0.4:11500".to_owned()];
        assert_eq!(
            destination(
                true,
                &paired,
                "http://localhost:11434",
                "gpt-oss:120b-cloud"
            ),
            Where::Elsewhere
        );
        // And it overrules the paired case too: a cloud model asked for through a Mac mini still
        // ends up at Ollama rather than at the Mac.
        assert_eq!(
            destination(
                false,
                &paired,
                "http://10.0.0.4:11500",
                "deepseek-v4-pro:cloud"
            ),
            Where::Elsewhere
        );
        // An ordinary model is unaffected, which is what makes this a narrowing rather than a
        // blanket suspicion.
        assert_eq!(
            destination(true, &paired, "http://localhost:11434", "qwen3:14b"),
            Where::Here
        );
    }
}
