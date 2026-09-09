//! Application settings — the choices that belong to the user's machine, not to a World.
//!
//! Kept deliberately small. A setting exists here only when the honest answer differs between
//! two people's machines and Epoch cannot work out which is right. Everything else should be a
//! good default (`CLAUDE.md`: "Great defaults beat endless configuration").

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{0}")]
    Malformed(String),
}

/// How many minutes a local model stays resident when the user allows several at once.
///
/// Long enough that a back-and-forth does not reload between every message, short enough that
/// a forgotten conversation does not hold a graphics card all afternoon.
pub const RESIDENT_MINUTES: u64 = 5;

/// How long a model stays resident when the user has **explicitly asked to hold it**.
///
/// Not a policy about idleness — [`RESIDENT_MINUTES`] is that. Here somebody pressed a button
/// that is still on their screen, and the answer they want is *until I press it again*.
///
/// It is a deadline rather than a decision, because no local server takes "forever" through a
/// number Epoch can send: Ollama and LM Studio both read seconds. A day is longer than any
/// session and short enough that a machine left running overnight is not still holding
/// somebody's card the next afternoon. The lamp reads the card either way, so a hold that
/// expires — or one the server evicts on its own — is visible rather than assumed.
pub const HELD_HOURS: u64 = 24;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct Settings {
    /// Whether **several** crew members may hold a model in memory at the same time.
    ///
    /// ## What this used to mean, and why that was wrong (corrected 2026-08-28)
    ///
    /// It used to decide whether a model was held **at all**: off meant every character released
    /// after every single answer. That conflated two different questions and answered the wrong
    /// one — measured on this machine, releasing after each answer costs about 19 s per message
    /// on a 27B (6 s reading the weights back off disk, 13 s re-evaluating the prompt from cold),
    /// and none of it is visible to anybody.
    ///
    /// The memory argument behind the old default is real and it was about *several*: one 14B
    /// held 4 GB and stayed resident five minutes with the whole crew idle, and 11.21 measured
    /// three runtimes each keeping a copy as a straight loss on a 12 GB card that also draws.
    /// **One model is not several.** The World runs one turn at a time, so holding whoever just
    /// spoke means at most one model resident — which is what the card had loaded a second
    /// earlier anyway.
    ///
    /// So the setting keeps its name and now governs only what its name says:
    ///
    /// - **Off** (the default): the last speaker's model stays; a *different* character speaking
    ///   releases theirs first. At most one.
    /// - **On**: every crew member's model stays warm. Replies are instant across the whole crew,
    ///   and each one holds its memory.
    ///
    /// Releasing still happens where something else needs the card — before a render, and while a
    /// Job is in flight — and where the user says so, by turning KEEP off.
    ///
    /// Hosted providers are unaffected either way: there is nothing of the user's to hold.
    pub concurrent_crew: bool,
    /// Extra sign-ins of the agent programs, beyond the one each program already had.
    ///
    /// ## Why this is a setting and not a measurement
    ///
    /// Everything else about an agent is asked of the program: is it installed, is it signed in,
    /// which account. What Epoch cannot ask is **how many accounts you want** and **what you call
    /// each one**, so those are the only two things stored — the id Epoch generated and the label
    /// the user typed.
    ///
    /// The default sign-in of each program is never listed here. It exists whether or not this
    /// file does, and writing it down would make it possible to delete something that cannot be
    /// deleted.
    ///
    /// Machine-scoped like the rest of this file: a sign-in lives in a keychain on one computer,
    /// so a list of them belongs beside `concurrentCrew` and not in a World.
    #[serde(default)]
    pub agent_accounts: Vec<AgentAccount>,
    /// What each local runtime is started with, keyed by its id: `ollama`, `llama_cpp`,
    /// `lm_studio`.
    ///
    /// ## Why this belongs to the machine and not to a character
    ///
    /// ADR-0026's one test: *does this survive changing the engine?* A GPU backend does not — it
    /// is a property of what is installed on this computer, in the same family as `num_gpu` and
    /// an endpoint. So it sits beside `concurrentCrew` rather than on anybody's Character, and a
    /// Character Pack carried to another machine cannot bring a backend that does not exist there.
    ///
    /// Absent is the ordinary state and means *the program decides*, which is what every one of
    /// them does with nothing set.
    #[serde(default)]
    pub runtimes: std::collections::BTreeMap<String, crate::models::runtimes::Preference>,
    /// Whether the user has let the World listen through this machine's microphone.
    ///
    /// ## Three states, and the third is the point
    ///
    /// `None` is **nobody has been asked**, and it is not a `no`. `Some(false)` is a refusal
    /// somebody made and Epoch has to keep, or the question comes back every time they press the
    /// button. `Some(true)` is consent.
    ///
    /// A `bool` would have made *unasked* mean *refused*, which is the inversion this codebase
    /// has already paid for twice — a gate with nothing behind it must stay open.
    ///
    /// ## Why it is stored at all
    ///
    /// Because Epoch answers WebView2's permission request on the user's behalf, and it may only
    /// do that having actually asked. The browser's own prompt appears over the World naming a
    /// URL — `http://tauri.localhost wants to use your microphones` — and it was missed, which
    /// looked exactly like a button that does nothing. Epoch asks in its own words instead; this
    /// is the answer it is allowed to give afterwards.
    ///
    /// Machine-scoped, beside `concurrentCrew`: a microphone belongs to a computer, not to a
    /// World and not to a character.
    #[serde(default)]
    pub microphone: Option<bool>,
    /// Which language the person at this machine speaks, when they have said.
    ///
    /// ## Why asking beats detecting, measured
    ///
    /// whisper's own default is English, so saying nothing translates somebody's Spanish
    /// (fixed 2026-09-06). `auto` is the honest replacement and it is a **guess on a short
    /// phrase**: *"hazme una tabla con esos datos"* came back as
    /// `Αυτοί, πρέπει να τα βλακουμε τα τάτια.` — Greek — and took three tries to land.
    ///
    /// ## And the machine's own locale is a reading of the wrong quantity
    ///
    /// `navigator.language` was the obvious default and it was measured before it was written:
    /// on the owner's machine it answers **`en-US`**, and he speaks Spanish. A Windows install
    /// language is a fact about the installer, not about the person.
    ///
    /// So `None` means *detect each time*, which is a real choice and the only honest default —
    /// and a transcription that had to guess **says which language it decided on**, so the
    /// setting is discoverable at the moment it is wanted rather than in a menu nobody opens.
    ///
    /// Machine-scoped, beside the microphone: the language somebody speaks belongs to the person
    /// at this keyboard, not to a World or a character.
    #[serde(default)]
    pub hearing_language: Option<String>,
    /// How many rounds of tools one turn may take.
    ///
    /// `None` is Epoch's own bound — 8, whose reasoning is *look → read → read → answer*, which
    /// was written before a character could have an MCP server's thirty-two tools attached.
    /// `Some(0)` is **no limit**, which is the user's to choose on their own machine.
    ///
    /// A machine setting rather than a Character parameter: it does not survive changing the
    /// engine (ADR-0026's one test). It is about how patient this installation is, and how much
    /// a model call costs here — the same family as `concurrentCrew`.
    #[serde(default)]
    pub tool_rounds: Option<usize>,
}

/// One sign-in the user added, as it is written down.
///
/// Deliberately thinner than `agents::Account`: this holds only what cannot be derived. The
/// directory is computed from the id (see `Settings::accounts`), because a stored path is a path
/// that can disagree with where the files actually are.
///
/// **No credential, ever.** The field does not exist on this type, which makes it a guarantee the
/// compiler holds rather than a review checklist — the same reason a Character has nowhere to put
/// an API key (ADR-0026).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAccount {
    /// Which program this is a sign-in of: `claude-code`, `codex`.
    pub kind: String,
    /// Stable id, generated by Epoch and never re-used: `claude-code-2`.
    ///
    /// A character's brain names this, so renaming the label must not change it — a label is
    /// something you fix on a Tuesday, and it must not silently take somebody's brain away.
    pub id: String,
    /// What the user called it. `trabajo`, `personal`, anything.
    pub label: String,
}

impl Settings {
    /// How long a model should stay loaded after it answers.
    ///
    /// **The same answer either way, and that is the correction.** Whether *several* may be held
    /// is a question about the other crew members, answered where the next turn starts — not by
    /// throwing away the model that just spoke. See [`Settings::concurrent_crew`].
    pub fn keep_loaded(&self) -> crate::provider::KeepLoaded {
        crate::provider::KeepLoaded::For(std::time::Duration::from_secs(RESIDENT_MINUTES * 60))
    }

    /// Read from the vault. Absent is the ordinary first-run state and yields the defaults.
    pub fn load(vault: &Path) -> Self {
        match std::fs::read_to_string(path(vault)) {
            Ok(raw) => toml::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, vault: &Path) -> Result<(), SettingsError> {
        let body =
            toml::to_string_pretty(self).map_err(|e| SettingsError::Malformed(e.to_string()))?;
        std::fs::create_dir_all(vault).map_err(|source| SettingsError::Write {
            path: vault.to_path_buf(),
            source,
        })?;
        let path = path(vault);
        // Atomically: a settings file found half-written is a machine that has forgotten what
        // the user chose about it, and the failure looks like Epoch resetting itself.
        epoch_secrets::atomically::replace(&path, body.as_bytes()).map_err(|why| {
            SettingsError::Write {
                path,
                source: std::io::Error::other(why),
            }
        })
    }
}

impl Settings {
    /// Every agent account on this machine: each program's own sign-in, then the added ones.
    ///
    /// The default of each program comes first and always exists — it is not in the file, because
    /// it is not a choice anybody made. An added account's directory is **derived from its id**
    /// rather than stored: one fact in one place cannot drift from itself.
    ///
    /// Under the vault deliberately. Measured: Codex refuses to create its PATH helpers when its
    /// home is inside a temporary directory and says so, so this has to be somewhere that lasts.
    pub fn accounts(&self, vault: &Path) -> Vec<crate::agents::Account> {
        let mut all = crate::agents::default_accounts();
        for added in &self.agent_accounts {
            all.push(crate::agents::Account {
                kind: added.kind.clone(),
                id: added.id.clone(),
                name: added.label.clone(),
                home: Some(account_home(vault, &added.id)),
            });
        }
        all
    }

    /// An id nothing on this machine has **ever** used, for a new sign-in of one program.
    ///
    /// ## Not merely one nothing is using
    ///
    /// A character's file names the id. Re-using one that was removed would attach that character
    /// to a *different* account without a word — the worst shape a wrong instrument can take, and
    /// one nothing on screen could explain.
    ///
    /// The first version of this counted upward over the current list and a test caught it
    /// immediately: remove `claude-code-2`, add another, and it handed `claude-code-2` straight
    /// back out.
    ///
    /// ## Where "ever" is recorded, and why it needed no new field
    ///
    /// Removing an account deliberately **leaves the agent's own directory alone** — deleting
    /// somebody's credentials because they tidied a row is not a thing a surface does quietly. So
    /// the directory's continued existence already *is* the record that the id was handed out.
    /// Measured rather than remembered, which is the same reason the home is derived instead of
    /// stored.
    pub fn next_account_id(&self, vault: &Path, kind: &str) -> String {
        let mut n = 2;
        loop {
            let candidate = format!("{kind}-{n}");
            let listed = self.agent_accounts.iter().any(|a| a.id == candidate);
            let on_disk = account_home(vault, &candidate).exists();
            if !listed && !on_disk {
                return candidate;
            }
            n += 1;
        }
    }
}

/// Where one added sign-in keeps its own configuration.
///
/// Epoch creates this directory and hands it to the agent through the agent's own environment
/// variable. It never reads what lands inside — the door opens, the key stays with the program.
pub fn account_home(vault: &Path, id: &str) -> PathBuf {
    vault.join("agents").join(id)
}

fn path(vault: &Path) -> PathBuf {
    vault.join("settings.toml")
}

#[cfg(test)]
mod tests {

    /// What a surface sending one field actually asks for.
    ///
    /// **This is not hypothetical.** `save_settings` took a whole `Settings` and the Launcher's
    /// concurrency checkbox sent `{ concurrentCrew }` and nothing else — so every press wrote a
    /// file with no agent accounts and no runtime preferences, silently deleting a second
    /// Claude Code sign-in and a chosen GPU backend. The same shape as `draws_in: None`, which
    /// this codebase has recorded once already.
    ///
    /// The defaults are right and the *whole-struct write* is the mistake, so this asserts what
    /// the wire really means rather than pretending the deserialiser is at fault.
    #[test]
    fn one_field_over_the_wire_is_a_whole_settings_with_everything_else_empty() {
        let partial: Settings =
            serde_json::from_str(r#"{"concurrentCrew":true}"#).expect("it deserialises");
        assert!(partial.concurrent_crew);
        assert!(
            partial.agent_accounts.is_empty(),
            "a sign-in would be erased by this write"
        );
        assert!(
            partial.runtimes.is_empty(),
            "a chosen backend would be erased by this write"
        );
        assert_eq!(partial.microphone, None);
    }

    /// Nobody has been asked is not a refusal.
    #[test]
    fn an_unasked_microphone_is_none_and_survives_a_round_trip() {
        let mut settings = Settings::default();
        assert_eq!(
            settings.microphone, None,
            "a fresh vault has asked nobody anything"
        );

        for said in [Some(true), Some(false), None] {
            settings.microphone = said;
            let back: Settings =
                toml::from_str(&toml::to_string_pretty(&settings).expect("writes")).expect("reads");
            assert_eq!(back.microphone, said, "{said:?} did not survive the file");
        }
    }
    use super::*;
    use crate::provider::KeepLoaded;

    struct Vault(PathBuf);

    impl Vault {
        fn new(name: &str) -> Self {
            let d = std::env::temp_dir().join(format!("epoch-settings-{name}"));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            Self(d)
        }
    }

    impl Drop for Vault {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// **The default holds one model, and holds it whatever this setting says.**
    ///
    /// Corrected 2026-08-28. `Never` used to be the default answer here, so the ordinary machine
    /// paid about 19 s reloading on every single message — measured, a 27B over llama.cpp — to
    /// protect against a cost that only *several* models have. The World runs one turn at a
    /// time, so what is held is what the card had loaded a second earlier anyway.
    ///
    /// The memory argument is intact and moved to where it belongs: with the setting off, a
    /// different character speaking releases the previous one first (`run_turn`), so at most one
    /// is ever resident. Releases for a render and for KEEP-off are unchanged.
    #[test]
    fn a_model_stays_loaded_whether_or_not_several_may_be() {
        let held = KeepLoaded::For(std::time::Duration::from_secs(RESIDENT_MINUTES * 60));
        let s = Settings::default();
        assert!(!s.concurrent_crew, "off is still the default");
        assert_eq!(s.keep_loaded(), held);
        assert_eq!(
            Settings {
                agent_accounts: Vec::new(),
                concurrent_crew: true,
                ..Settings::default()
            }
            .keep_loaded(),
            held,
            "the setting decides how many are held, never whether one is"
        );
    }

    #[test]
    fn a_choice_survives_a_round_trip_and_absence_is_not_a_failure() {
        let v = Vault::new("round");
        assert!(
            !Settings::load(&v.0).concurrent_crew,
            "absent means defaults"
        );

        Settings {
            agent_accounts: Vec::new(),
            concurrent_crew: true,
            ..Settings::default()
        }
        .save(&v.0)
        .unwrap();
        assert!(Settings::load(&v.0).concurrent_crew);
    }

    #[test]
    fn a_broken_settings_file_falls_back_rather_than_stopping_epoch() {
        // Settings are a preference, not a contract. Refusing to start over one would be worse
        // than any wrong preference could be.
        let v = Vault::new("broken");
        std::fs::write(v.0.join("settings.toml"), "this is not = valid [[[").unwrap();
        assert_eq!(Settings::load(&v.0), Settings::default());
    }

    /// The default sign-in of each program exists whether or not anything was written down.
    ///
    /// Deliberately not stored: writing it into the file would make it possible to delete
    /// something that cannot be deleted.
    #[test]
    fn a_fresh_machine_already_has_one_account_of_each_program() {
        let vault = Vault::new("accounts-fresh");
        let all = Settings::default().accounts(&vault.0);
        assert_eq!(all.len(), 3, "{all:?}");
        assert!(all.iter().all(|a| a.id == a.kind && a.home.is_none()));
    }

    /// An added account's directory is derived from its id, never stored.
    ///
    /// One fact in one place cannot drift from itself — and a stored path is a path that can
    /// disagree with where the files actually are.
    #[test]
    fn an_added_account_gets_its_own_folder_under_the_vault() {
        let vault = Vault::new("accounts-added");
        let settings = Settings {
            agent_accounts: vec![AgentAccount {
                kind: "claude-code".into(),
                id: "claude-code-2".into(),
                label: "work".into(),
            }],
            ..Settings::default()
        };
        let all = settings.accounts(&vault.0);
        let added = all.last().expect("the added one");
        assert_eq!(added.id, "claude-code-2");
        // The label is what a person reads. It is the user's word and never a measurement.
        assert_eq!(added.name, "work");
        assert_eq!(
            added.home.as_deref(),
            Some(account_home(&vault.0, "claude-code-2").as_path())
        );
        // And the program's own sign-in is still there, untouched and first.
        assert!(all
            .iter()
            .any(|a| a.id == "claude-code" && a.home.is_none()));
    }

    /// An id is never re-used, even after one is removed.
    ///
    /// **Because a character's file names the id.** Removing the second of three and adding
    /// another must not hand out an id somebody's crew still points at — that character would
    /// silently start working as a different account.
    #[test]
    fn an_id_that_was_handed_out_is_not_handed_out_again() {
        let vault = Vault::new("accounts-ids");
        let mut settings = Settings::default();

        let first = settings.next_account_id(&vault.0, "claude-code");
        assert_eq!(first, "claude-code-2");
        std::fs::create_dir_all(account_home(&vault.0, &first)).unwrap();
        settings.agent_accounts.push(AgentAccount {
            kind: "claude-code".into(),
            id: first,
            label: "work".into(),
        });

        let second = settings.next_account_id(&vault.0, "claude-code");
        assert_eq!(second, "claude-code-3");
        std::fs::create_dir_all(account_home(&vault.0, &second)).unwrap();
        settings.agent_accounts.push(AgentAccount {
            kind: "claude-code".into(),
            id: second,
            label: "personal".into(),
        });

        // **The one this test was written for.** Remove the middle account and the next id must
        // still move forward: a character's file may still name `claude-code-2`, and handing that
        // id to a different sign-in would move them to another account without a word.
        settings.agent_accounts.retain(|a| a.id != "claude-code-2");
        assert_eq!(
            settings.next_account_id(&vault.0, "claude-code"),
            "claude-code-4",
            "the removed account's folder is still there, and that is the record"
        );
    }

    /// Accounts survive a round trip through the file, and nothing else changes.
    #[test]
    fn what_was_added_is_still_there_after_a_restart() {
        let vault = Vault::new("accounts-roundtrip");
        let settings = Settings {
            concurrent_crew: true,
            microphone: Some(true),
            hearing_language: Some("es".into()),
            tool_rounds: Some(0),
            runtimes: Default::default(),
            agent_accounts: vec![AgentAccount {
                kind: "codex".into(),
                id: "codex-2".into(),
                label: "personal".into(),
            }],
        };
        settings.save(&vault.0).expect("written");
        assert_eq!(Settings::load(&vault.0), settings);
    }
}
