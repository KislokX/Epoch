//! The Trust store, and the gate every execution passes through (ADR-0009).
//!
//! The **decision** is a pure function in the Kernel. This holds the two things it needs that
//! live on disk — what mode each World is in, and what the user has already decided — and it
//! is the only door into them.
//!
//! ## A broken trust file is not a preference
//!
//! [`Settings`](crate::Settings) falls back to defaults when its file will not parse, because a
//! wrong preference is better than refusing to start. Trust cannot do that. Falling back to
//! defaults would silently discard every `Never allow` the user ever set, and the failure would
//! look exactly like everything working.
//!
//! So a malformed trust file is **reported and the World runs with nothing allowed beyond the
//! defaults it can prove** — the file is never overwritten, so the user can fix it and get
//! their decisions back.
//!
//! ## Nothing here can be changed by a model
//!
//! [`TrustStore::remember`] is reachable only from a user command. No capability, no provider
//! and no tool result has a path to it. That is ADR-0009's stated mitigation against prompt
//! injection escalating trust, and it holds because there is no other door.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use epoch_kernel::{
    decide, offerable, Arguments, Autonomy, CharacterId, Descriptor, Policy, Situation, Verdict,
};
use serde::{Deserialize, Serialize};

use crate::capability::{Capability, CapabilityError, CapabilityRegistry};

#[derive(Debug, thiserror::Error)]
pub enum TrustError {
    #[error("cannot write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{0}")]
    Malformed(String),
}

/// What the user has decided, and how much rope each World has.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustStore {
    /// Autonomy per World, by World id.
    ///
    /// Per World rather than global on purpose: "this is my scratch project" and "this is the
    /// thing that pays my rent" are different answers, and a single dial would force the
    /// cautious one onto both.
    #[serde(default)]
    mode: BTreeMap<String, Autonomy>,
    /// Standing decisions. Ordered as the user made them; precedence is by scope, not by order.
    #[serde(default)]
    policies: Vec<Policy>,
    /// Why the stored decisions are missing, when they are. `None` means the file was fine.
    ///
    /// Not serialised — it describes this load, not the user's choices.
    #[serde(skip)]
    problem: Option<String>,
}

impl TrustStore {
    /// Read from the vault.
    ///
    /// Absent is the ordinary first-run state. **Malformed is not** — see the module docs.
    pub fn load(vault: &Path) -> Self {
        let path = path(vault);
        let raw = match std::fs::read_to_string(&path) {
            Ok(raw) => raw,
            // Nothing decided yet. Every World gets the default mode and asks about everything
            // that is not routine.
            Err(_) => return Self::default(),
        };
        match toml::from_str::<TrustStore>(&raw) {
            Ok(store) => store,
            Err(err) => Self {
                problem: Some(format!(
                    "{} could not be read ({err}); Epoch is running with no stored trust \
                     decisions. Nothing has been overwritten — fix the file to get them back.",
                    path.display()
                )),
                ..Self::default()
            },
        }
    }

    pub fn save(&self, vault: &Path) -> Result<(), TrustError> {
        // Refusing to write over decisions we could not read is the point of the whole
        // fail-closed treatment above; writing here would destroy them.
        if let Some(problem) = &self.problem {
            return Err(TrustError::Malformed(problem.clone()));
        }
        let body =
            toml::to_string_pretty(self).map_err(|e| TrustError::Malformed(e.to_string()))?;
        std::fs::create_dir_all(vault).map_err(|source| TrustError::Write {
            path: vault.to_path_buf(),
            source,
        })?;
        let path = path(vault);
        std::fs::write(&path, body).map_err(|source| TrustError::Write { path, source })
    }

    /// What went wrong reading the stored decisions, if anything. Shown, never swallowed.
    pub fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }

    /// How much rope this World has. Unset means the default, which is Builder.
    pub fn mode(&self, world: &str) -> Autonomy {
        self.mode.get(world).copied().unwrap_or_default()
    }

    pub fn set_mode(&mut self, world: &str, mode: Autonomy) {
        self.mode.insert(world.to_owned(), mode);
    }

    pub fn policies(&self) -> &[Policy] {
        &self.policies
    }

    /// Record a standing decision. **Only ever called from a user command.**
    ///
    /// Replaces an existing decision at the same capability and scope rather than stacking, so
    /// changing your mind is changing your mind and not adding a second opinion.
    pub fn remember(&mut self, policy: Policy) {
        self.policies
            .retain(|p| !(p.capability == policy.capability && p.scope == policy.scope));
        self.policies.push(policy);
    }

    /// Take a standing decision back. Also only from a user command.
    pub fn forget(&mut self, capability: &str, scope: &epoch_kernel::Scope) {
        self.policies
            .retain(|p| !(p.capability == capability && &p.scope == scope));
    }

    /// Forget everything this World ever decided — its mode and every policy scoped to it.
    ///
    /// **Security rather than tidiness.** Ids are the user's to choose, so a World made later
    /// with the same id would otherwise inherit permissions granted to a different World, and
    /// nobody would see it happen.
    pub fn forget_world(&mut self, world: &str) {
        self.mode.remove(world);
        let scope = epoch_kernel::Scope::World(world.to_owned());
        self.policies.retain(|p| p.scope != scope);
    }

    /// Forget every decision about capabilities that no longer exist, at any scope.
    ///
    /// The one case where a standing answer is dropped without the user saying so, and it is the
    /// case where keeping it would be the dangerous thing. A decision is remembered *by
    /// capability id*, and an outside server's ids are built from a name the user chose — so a
    /// second, different server installed under that name would inherit an approval given to the
    /// first. The user would have approved one third party's `write` forever and be silently
    /// held to it for another's.
    ///
    /// Nothing downstream could catch that: ADR-0008 deliberately makes an MCP tool
    /// indistinguishable from any other capability, which is what makes one permission system
    /// possible and what makes this removal necessary.
    pub fn forget_all(&mut self, capabilities: &[String]) {
        self.policies
            .retain(|p| !capabilities.contains(&p.capability));
    }
}

fn path(vault: &Path) -> PathBuf {
    vault.join("trust.toml")
}

/// The mode a turn actually runs at, from the three places an answer can come from.
///
/// In order: what this Quest was set to, what was chosen before a Quest existed, and the
/// World's standing setting. The order is the design — a choice made inside a conversation is
/// about that conversation, and the World's setting is what applies when nobody has made one.
///
/// ## Why this is a function rather than three lines at each call
///
/// It was three lines at each call, in three places, and one of them was wrong. The turn
/// resolved it correctly and so did the surface; the **agent door** read the World's setting
/// alone, as though it were the only source.
///
/// The consequence was invisible and total: the composer's dropdown said Auto, the agent was
/// launched at Auto, and every Epoch tool it called was judged at the World's default instead.
/// Each call asked — during an agent's turn, when nobody is watching for a prompt — and came
/// back refused. From the outside it read as Epoch refusing its own tools, and the character
/// reported it as *"la llamada fue rechazada por el permiso de Epoch"*.
///
/// It stayed hidden because a standing decision hides it: the character whose calls were
/// already allowed never asked, so the same door worked for one crew member and not the other.
/// A rule written down three times is three chances to write two of them.
pub fn in_force(quest: Option<Autonomy>, prepared: Option<Autonomy>, world: Autonomy) -> Autonomy {
    quest.or(prepared).unwrap_or(world)
}

/// The gate. Everything with an effect goes through here, whoever asked for it.
pub struct TrustEngine<'a> {
    store: &'a TrustStore,
    character: &'a CharacterId,
    world: &'a str,
    /// The mode **this turn** runs at, resolved by [`in_force`] before the gate is built.
    ///
    /// Taken rather than derived, and that is the fix. It used to read `store.mode(world)`
    /// itself, which is only the last of three sources — so a mode chosen in a conversation
    /// reached the agent path and never reached the gate. Every model-brained character's MODE
    /// dropdown was decoration: it wrote a value nothing read.
    ///
    /// A parameter forces each caller to say which mode it is running at. That is exactly the
    /// thing that was being got wrong silently, and a silent default is what let it happen.
    mode: Autonomy,
}

impl<'a> TrustEngine<'a> {
    pub fn new(
        store: &'a TrustStore,
        character: &'a CharacterId,
        world: &'a str,
        mode: Autonomy,
    ) -> Self {
        Self {
            store,
            character,
            world,
            mode,
        }
    }

    fn situation(&self) -> Situation<'a> {
        Situation {
            character: self.character,
            world: self.world,
            mode: self.mode,
        }
    }

    /// What this character may even be *told* they have, this turn.
    ///
    /// Three filters, and all three must pass: the capability exists, the character asked for
    /// it (ADR-0026 — authored intent is the tool set), and trust would not refuse it outright.
    ///
    /// Something never offered cannot be argued into. A model that is not told about
    /// `delete_file` does not spend three rounds trying to reason its way to it.
    pub fn offer<'r>(
        &self,
        registry: &'r CapabilityRegistry,
        requested: impl IntoIterator<Item = &'r str>,
    ) -> Vec<Descriptor> {
        let asked = registry.requested(requested);
        let permitted = offerable(&asked, &self.situation(), self.store.policies());
        asked
            .into_iter()
            .filter(|d| permitted.contains(d.id.as_str()))
            .collect()
    }

    /// Capabilities this build has, that this character was **not** given.
    ///
    /// Told to the model so it can ask for one rather than concluding the job is impossible.
    /// Knowing something exists is not having it: reaching for one stops the turn and puts the
    /// question to the user (`Stop::NeedsCapability`), who is the only one who can answer it.
    pub fn withheld<'r>(
        &self,
        registry: &'r CapabilityRegistry,
        requested: impl IntoIterator<Item = &'r str>,
    ) -> Vec<Descriptor> {
        let offered = self.offer(registry, requested);
        registry
            .describe_all()
            .into_iter()
            .filter(|d| !offered.iter().any(|o| o.id == d.id))
            .collect()
    }

    /// Judge one proposed call: build the explanation, then decide.
    ///
    /// The explanation comes from the capability *before* the verdict, so a refusal and an
    /// approval describe the same thing in the same words — and so what the user approves is
    /// what runs.
    pub fn judge(
        &self,
        capability: &dyn Capability,
        arguments: &Arguments,
    ) -> Result<Verdict, CapabilityError> {
        let descriptor = capability.describe();
        let explanation = capability.explain(arguments)?;
        Ok(decide(
            &descriptor,
            &explanation,
            &self.situation(),
            self.store.policies(),
        ))
    }

    pub fn mode(&self) -> Autonomy {
        self.store.mode(self.world)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use epoch_kernel::{
        CapabilityId, Descriptor, Effect, Explanation, Parameter, Reversal, Scope, Value, ValueKind,
    };

    use crate::capability::Outcome;

    struct Vault(PathBuf);

    impl Vault {
        fn new(name: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let d = std::env::temp_dir().join(format!("epoch-trust-{name}-{n}"));
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

    struct Reader;
    impl Capability for Reader {
        fn describe(&self) -> Descriptor {
            Descriptor::observing(CapabilityId::new("read_file").unwrap(), "Read a file.").taking([
                Parameter::required("path", ValueKind::Text, "Path in the project."),
            ])
        }
        fn run(&self, _a: &Arguments) -> Result<Outcome, CapabilityError> {
            Ok(Outcome::told("…"))
        }
    }

    struct Deleter;
    impl Capability for Deleter {
        fn describe(&self) -> Descriptor {
            Descriptor::acting(
                CapabilityId::new("delete_file").unwrap(),
                "Delete a file.",
                [Effect::Deletes],
                Reversal::Permanent,
            )
            .taking([Parameter::required(
                "path",
                ValueKind::Text,
                "Path in the project.",
            )])
        }
        fn explain(&self, a: &Arguments) -> Result<Explanation, CapabilityError> {
            let path = a.text("path").map_err(CapabilityError::BadArguments)?;
            Ok(Explanation::of(
                &self.describe(),
                format!("delete `{path}`"),
            ))
        }
        fn run(&self, _a: &Arguments) -> Result<Outcome, CapabilityError> {
            Ok(Outcome::made("deleted", "a/file", "one file removed"))
        }
    }

    fn registry() -> CapabilityRegistry {
        let mut r = CapabilityRegistry::new();
        r.register(Box::new(Reader));
        r.register(Box::new(Deleter));
        r
    }

    fn robo() -> CharacterId {
        CharacterId::new("robo").unwrap()
    }

    #[test]
    fn absence_is_the_first_run_state_and_costs_nothing() {
        let v = Vault::new("absent");
        let store = TrustStore::load(&v.0);
        assert_eq!(store.mode("archipelago"), Autonomy::Manual);
        assert!(store.policies().is_empty());
        assert_eq!(store.problem(), None);
    }

    #[test]
    fn a_broken_trust_file_is_reported_and_never_overwritten() {
        // Unlike Settings, which falls back silently. Doing that here would discard every
        // "never allow" the user ever set, and look exactly like everything working.
        let v = Vault::new("broken");
        std::fs::write(path(&v.0), "policies = [[[ not toml").unwrap();

        let store = TrustStore::load(&v.0);
        assert!(store.problem().unwrap().contains("could not be read"));
        assert!(store.policies().is_empty(), "nothing is assumed");

        // And saving over it is refused, so a fixable file stays fixable.
        assert!(store.save(&v.0).is_err());
        let still_there = std::fs::read_to_string(path(&v.0)).unwrap();
        assert!(still_there.contains("not toml"));
    }

    #[test]
    fn decisions_survive_a_round_trip() {
        let v = Vault::new("round");
        let mut store = TrustStore::default();
        store.set_mode("archipelago", Autonomy::Auto);
        store.remember(Policy::deny("delete_file", Scope::Character(robo())));
        store.save(&v.0).unwrap();

        let read = TrustStore::load(&v.0);
        assert_eq!(read.mode("archipelago"), Autonomy::Auto);
        assert_eq!(
            read.mode("default"),
            Autonomy::Manual,
            "another World is untouched"
        );
        assert_eq!(read.policies().len(), 1);
    }

    #[test]
    fn changing_your_mind_replaces_rather_than_stacks() {
        let mut store = TrustStore::default();
        store.remember(Policy::allow("delete_file", Scope::Everywhere));
        store.remember(Policy::deny("delete_file", Scope::Everywhere));
        assert_eq!(store.policies().len(), 1);
        assert_eq!(store.policies()[0].decision, epoch_kernel::Decision::Deny);

        store.forget("delete_file", &Scope::Everywhere);
        assert!(store.policies().is_empty());
    }

    #[test]
    fn a_character_is_offered_what_they_asked_for_and_trust_permits() {
        let registry = registry();
        let store = TrustStore::default();
        let who = robo();

        let trust = TrustEngine::new(&store, &who, "archipelago", store.mode("archipelago"));
        let offered = trust.offer(&registry, ["read_file", "delete_file"]);
        assert_eq!(
            offered.len(),
            2,
            "Builder may do both, asking before the delete"
        );

        // Asking for something nobody built is not an error, it is simply not offered.
        assert_eq!(trust.offer(&registry, ["web_search"]).len(), 0);
    }

    #[test]
    fn no_mode_shrinks_the_offer_because_a_mode_decides_when_to_ask_not_what_exists() {
        // The distinction that survived removing Plan: a mode says whether something stops to
        // ask, never whether it is on the table. Only a standing `Deny` takes something off.
        let registry = registry();
        let who = robo();

        for mode in Autonomy::ALL {
            let mut store = TrustStore::default();
            store.set_mode("archipelago", mode);
            let offered = TrustEngine::new(&store, &who, "archipelago", store.mode("archipelago"))
                .offer(&registry, ["read_file", "delete_file"]);
            assert_eq!(offered.len(), 2, "{mode}");
        }
    }

    #[test]
    fn a_denied_capability_is_never_mentioned_to_the_model() {
        let registry = registry();
        let mut store = TrustStore::default();
        store.remember(Policy::deny("delete_file", Scope::Character(robo())));
        let who = robo();

        let offered = TrustEngine::new(&store, &who, "archipelago", store.mode("archipelago"))
            .offer(&registry, ["read_file", "delete_file"]);
        assert_eq!(offered.len(), 1);

        // And another crew member is unaffected — the decision was about Robo.
        let mage = CharacterId::new("mage").unwrap();
        let theirs = TrustEngine::new(&store, &mage, "archipelago", store.mode("archipelago"))
            .offer(&registry, ["read_file", "delete_file"]);
        assert_eq!(theirs.len(), 2);
    }

    #[test]
    fn judging_uses_the_capabilitys_own_words() {
        let store = TrustStore::default();
        let who = robo();
        let trust = TrustEngine::new(&store, &who, "archipelago", store.mode("archipelago"));

        let args = Arguments::new().with("path", Value::Text("notes/old.md".into()));
        match trust.judge(&Deleter, &args).unwrap() {
            Verdict::Ask(shown) => {
                // The capability's own explanation, not a generic one built here.
                assert_eq!(shown.what, "delete `notes/old.md`");
                assert!(shown.reversal.is_permanent());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn bad_arguments_are_caught_before_the_user_is_asked_anything() {
        // Nobody should be asked to approve a call that could not have worked.
        let store = TrustStore::default();
        let who = robo();
        let trust = TrustEngine::new(&store, &who, "archipelago", store.mode("archipelago"));
        let err = trust.judge(&Deleter, &Arguments::new()).unwrap_err();
        assert!(matches!(err, CapabilityError::BadArguments(ref why) if why.contains("required")));
    }

    #[test]
    fn reading_never_asks_and_deleting_always_does_until_told_otherwise() {
        let store = TrustStore::default();
        let who = robo();
        let trust = TrustEngine::new(&store, &who, "archipelago", store.mode("archipelago"));
        let args = Arguments::new().with("path", Value::Text("a.md".into()));

        assert!(trust.judge(&Reader, &args).unwrap().is_allowed());
        assert!(matches!(
            trust.judge(&Deleter, &args).unwrap(),
            Verdict::Ask(_)
        ));

        let mut allowed = TrustStore::default();
        allowed.remember(Policy::allow(
            "delete_file",
            Scope::World("archipelago".into()),
        ));
        let trust = TrustEngine::new(&allowed, &who, "archipelago", allowed.mode("archipelago"));
        assert!(trust.judge(&Deleter, &args).unwrap().is_allowed());
    }
    #[test]
    fn a_removed_servers_standing_answers_go_with_it() {
        // The one place a standing decision is dropped without the user saying so, and it is
        // the place where keeping it would be the dangerous thing.
        //
        // A decision is remembered by *capability id*, and an outside server's ids are built
        // from a name the user chose. Forget `spotify` and install a different Spotify server
        // under the same name — exactly what somebody does when the first one turns out not to
        // work — and it would inherit an approval given to a program the user has never seen.
        // Nothing downstream could catch it: ADR-0008 deliberately makes an MCP tool
        // indistinguishable from any other capability.
        let mut store = TrustStore::default();
        store.remember(epoch_kernel::Policy::allow(
            "spotify_login",
            Scope::World("default".into()),
        ));
        store.remember(epoch_kernel::Policy::deny("mcp:spotify", Scope::Everywhere));
        store.remember(epoch_kernel::Policy::allow(
            "read_file",
            Scope::World("default".into()),
        ));
        // A neighbour whose name merely begins the same way keeps everything.
        store.remember(epoch_kernel::Policy::allow(
            "spotify_beta_login",
            Scope::World("default".into()),
        ));

        store.forget_all(&["spotify_login".to_owned(), "mcp:spotify".to_owned()]);

        let left: Vec<&str> = store
            .policies()
            .iter()
            .map(|p| p.capability.as_str())
            .collect();
        assert_eq!(left, vec!["read_file", "spotify_beta_login"]);
    }
    #[test]
    fn the_mode_in_force_prefers_the_conversation_over_the_world() {
        // The order is the design: a choice made inside a conversation is about that
        // conversation. The World's setting is the answer when nobody has made one.
        assert_eq!(
            in_force(Some(Autonomy::Auto), None, Autonomy::Manual),
            Autonomy::Auto,
            "the Quest's own choice wins"
        );
        assert_eq!(
            in_force(None, Some(Autonomy::Auto), Autonomy::Manual),
            Autonomy::Auto,
            "a choice made before a Quest existed still counts"
        );
        assert_eq!(
            in_force(Some(Autonomy::Manual), Some(Autonomy::Auto), Autonomy::Auto),
            Autonomy::Manual,
            "the Quest outranks the one prepared before it"
        );
        assert_eq!(
            in_force(None, None, Autonomy::AcceptEdits),
            Autonomy::AcceptEdits,
            "the World is the fallback, not the answer"
        );
    }
    #[test]
    fn the_gate_runs_at_the_mode_it_was_given_not_the_one_in_the_file() {
        // The defect, as the property that failed. `Autonomy` reached the gate by being read
        // back out of the World's file, so a mode chosen inside a conversation could not change
        // anything: the composer wrote `Auto` into the Quest and the gate went on judging at the
        // World's default. For a model-brained character the dropdown was decoration; for an
        // agent it was worse, because the agent *was* launched at Auto and its calls were then
        // refused by a gate running at something else.
        //
        // Nothing about the store changes here. What changed is that the mode is passed in, so
        // the two can be different and the given one wins.
        let store = TrustStore::default();
        assert_eq!(
            store.mode("archipelago"),
            Autonomy::default(),
            "the file says nothing, which is the situation this is about"
        );
        let who = CharacterId::new("mage").unwrap();

        let asked = TrustEngine::new(&store, &who, "archipelago", Autonomy::default());
        let silent = TrustEngine::new(&store, &who, "archipelago", Autonomy::Auto);
        let arguments = Arguments::new().with("path", Value::Text("old.md".into()));

        assert!(
            matches!(asked.judge(&Deleter, &arguments), Ok(Verdict::Ask(_))),
            "the World's own setting still asks"
        );
        assert!(
            matches!(silent.judge(&Deleter, &arguments), Ok(Verdict::Allowed(_))),
            "and a turn told it runs at Auto does not"
        );
    }
}
