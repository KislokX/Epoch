//! Other computers a turn may go to (ADR-0029).
//!
//! Split out of `state.rs` unchanged: pairing, grants, and what a paired machine reports.

use super::*;

/// File a machine on the roster and mint the bearer it will carry.
///
/// **The only way a bond comes into existence.** Both the panel (a person typing a code into
/// Epoch) and the pairing door (a machine redeeming one over the network) come through here, so
/// there are not two kinds of paired machine with two sets of rules.
///
/// Free of `&self` on purpose: the pairing door runs on its own thread and must be able to
/// enroll without holding anything the turn loop also holds — the same rule that cost a frozen
/// window once already (`std::sync::Mutex` is not reentrant).
pub(crate) fn enroll(
    name: &str,
    address: &str,
    fingerprint: Option<String>,
    grants: &[epoch_engine::Grant],
) -> Result<epoch_kernel::Welcome, String> {
    if address.trim().is_empty() {
        return Err("a machine needs an address to be reached at".to_owned());
    }
    let vault = vault_dir();
    let mut pairings = epoch_engine::Pairings::load(&vault);
    let id = pairings.add(
        name,
        address,
        fingerprint,
        grants.to_vec(),
        epoch_engine::now_ms(),
    );
    pairings.save(&vault)?;

    // The bearer, where the door token lives — DPAPI on Windows. The roster is a file a person
    // can read, and a file a person can read is not where a bearer belongs.
    let secret = epoch_engine::endpoint::Token::fresh().as_str().to_owned();
    epoch_engine::secrets::Secrets::at(&vault).put(
        &epoch_engine::pairing::secret_name(&id),
        &epoch_kernel::Secret::new(secret.clone()),
    )?;
    Ok(epoch_kernel::Welcome { id, secret })
}

/// A repository id, with whatever prefix a pull string carries stripped off.
///
/// `hf.co/unsloth/x` is what Ollama takes; `unsloth/x` is what `hf` takes. One of them has to
/// give, and it is not the user.
pub(crate) fn bare(repo: &str) -> &str {
    repo.trim()
        .trim_start_matches("hf.co/")
        .trim_start_matches("huggingface.co/")
}

/// One image studio, by the id a surface sent.
///
/// A closed set, like `runtime_named`: an id naming something this build cannot start is a press
/// that can only fail, and it should fail here with a sentence rather than later with nothing.
pub(crate) fn studio_named(id: &str) -> Result<epoch_engine::models::studio::Studio, String> {
    epoch_engine::models::studio::Studio::ALL
        .into_iter()
        .find(|studio| studio.id() == id.trim())
        .ok_or_else(|| format!("'{id}' is not an image studio this build knows"))
}

/// One runtime, by the id a surface sends back.
///
/// Refused rather than defaulted: a command addressed to something this build does not know
/// should say so, not quietly act on the first entry.
pub(crate) fn runtime_named(id: &str) -> Result<epoch_engine::models::runtimes::Runtime, String> {
    epoch_engine::models::runtimes::Runtime::ALL
        .into_iter()
        .find(|r| r.id() == id)
        .ok_or_else(|| format!("'{id}' is not a runtime this build knows"))
}

impl World {
    /// Every machine paired with this Host, and what each may be.
    pub fn bridges(&self) -> Vec<epoch_engine::Paired> {
        epoch_engine::Pairings::load(&vault_dir()).all().to_vec()
    }

    /// Reach out to a machine that is showing a code, and file it.
    ///
    /// **The other direction**, for a network whose router will not let the remote start a
    /// connection. Everything after the exchange is identical to the ordinary way round — same
    /// roster, same bearer store, same grants — which is why it goes through the same `enroll`.
    pub fn enrol_machine(
        &self,
        address: &str,
        code: &str,
        grants: &[String],
    ) -> Result<String, String> {
        let met = epoch_engine::pairing::enrol(address, code)?;

        let grants: Vec<epoch_engine::Grant> = grants
            .iter()
            .filter_map(|id| epoch_engine::Grant::from_id(id))
            .collect();

        // The roster entry, exactly as the other direction files one. The secret is the one that
        // machine was just given, rather than a fresh one: minting a second here would leave the
        // two ends holding different bearers.
        let vault = vault_dir();
        let mut pairings = epoch_engine::Pairings::load(&vault);
        let id = pairings.add(
            &met.name,
            &met.address,
            Some(met.fingerprint),
            grants,
            epoch_engine::now_ms(),
        );
        pairings.save(&vault)?;
        self.secrets().put(
            &epoch_engine::pairing::secret_name(&id),
            &epoch_kernel::Secret::new(met.secret),
        )?;
        // A machine paired a moment ago has to be usable a moment later. Without this it was
        // filed, listed, and unreachable until Epoch restarted.
        self.roster_changed();
        Ok(id)
    }

    /// Finish a pairing: the code is checked, a long secret is minted and kept.
    ///
    /// **The code buys the secret and is then spent.** Leaving it usable would mean a code read
    /// off a screen keeps working after the pairing it was for.
    ///
    /// This is the completion of the direction where the *machine* dials in, so the fingerprint
    /// comes from its `Hello` rather than from a handshake this side started. It is required:
    /// a roster entry with nothing to pin is one the Bridge will refuse on every turn, and
    /// filing it would be adding a machine that looks paired and can never be reached.
    pub fn pair(
        &self,
        code: &str,
        name: &str,
        address: &str,
        fingerprint: Option<String>,
        grants: &[String],
    ) -> Result<String, String> {
        {
            let mut held = guard(&self.pairing);
            let matched = held.as_ref().is_some_and(|held| held.matches(code));
            if !matched {
                return Err(
                    "that code is wrong or has expired. Show a new one and try again.".to_owned(),
                );
            }
            *held = None;
        }
        if address.trim().is_empty() {
            return Err("a machine needs an address to be reached at".to_owned());
        }
        let Some(fingerprint) = fingerprint.filter(|it| !it.trim().is_empty()) else {
            return Err(
                "that machine did not say which certificate it answers with, so there would be                  nothing to recognise it by. Pair from the panel instead."
                    .to_owned(),
            );
        };

        let grants: Vec<epoch_engine::Grant> = grants
            .iter()
            .filter_map(|id| epoch_engine::Grant::from_id(id))
            .collect();

        let welcome = enroll(name, address, Some(fingerprint), &grants)?;
        // The other direction files the same roster entry, so it owes the same rebuild.
        self.roster_changed();
        Ok(welcome.id)
    }

    /// Change what a paired machine may be, without unpairing it.
    pub fn set_bridge_grants(&self, id: &str, grants: &[String]) -> Result<(), String> {
        let vault = vault_dir();
        let mut pairings = epoch_engine::Pairings::load(&vault);
        let grants: Vec<epoch_engine::Grant> = grants
            .iter()
            .filter_map(|g| epoch_engine::Grant::from_id(g))
            .collect();
        if !pairings.set_grants(id, grants) {
            return Err(format!("no paired machine has the id '{id}'"));
        }
        pairings.save(&vault)?;
        // Only machines that may compute become Providers, so taking that grant away has to
        // take the Provider away with it.
        self.roster_changed();
        Ok(())
    }

    /// Unpair a machine: it leaves the roster **and** its secret is forgotten.
    ///
    /// Both, or unpairing would leave a working bearer behind for a machine the user believes
    /// is gone.
    pub fn unpair(&self, id: &str) -> Result<(), String> {
        let vault = vault_dir();
        let mut pairings = epoch_engine::Pairings::load(&vault);
        if !pairings.remove(id) {
            return Err(format!("no paired machine has the id '{id}'"));
        }
        pairings.save(&vault)?;
        self.secrets()
            .forget(&epoch_engine::pairing::secret_name(id))?;
        // Gone from the roster and gone from what can think. It stayed in the list otherwise —
        // a machine the user believes they removed, still being offered and still being asked.
        self.roster_changed();
        Ok(())
    }

    /// What has been answered about turns leaving this machine, and what has not.
    pub fn disclosures(&self) -> Vec<(String, Option<bool>, &'static str)> {
        let kept = epoch_engine::Disclosures::load(&vault_dir());
        [epoch_engine::Where::Yours, epoch_engine::Where::Elsewhere]
            .into_iter()
            .map(|going| (going.id().to_owned(), kept.allowed(going), going.asks()))
            .collect()
    }

    /// Answer the disclosure question for one destination, or take the answer back.
    pub fn answer_disclosure(&self, going: &str, allowed: Option<bool>) -> Result<(), String> {
        let going = epoch_engine::Where::from_id(going)
            .ok_or_else(|| format!("'{going}' is not somewhere a turn can go"))?;
        let vault = vault_dir();
        let mut kept = epoch_engine::Disclosures::load(&vault);
        match allowed {
            Some(answer) => kept.answer(going, answer),
            None => kept.forget(going),
        }
        kept.save(&vault)
    }

    /// What this machine is, measured now.
    pub fn machine(&self) -> epoch_engine::Machine {
        epoch_engine::Machine::measure()
    }
}
