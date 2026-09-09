//! The people, and what thinks for them (ADR-0023, ADR-0026, ADR-0027).
//!
//! Split out of `state.rs` unchanged: the vault editor over a Character, the sign-ins
//! an agent brain can name, and the surface a Provider declares for its parameters.

use super::*;

impl World {
    /// Which agents this machine has.
    ///
    /// Running each program is what makes this a fact rather than a list — the same rule a
    /// Provider's probe follows, and the reason a surface can say *Claude Code is not installed*
    /// instead of simply not offering it. Availability is two questions with two different
    /// fixes, so an installed agent costs two processes: `--version`, then `auth status`.
    ///
    /// **Measured once and remembered, because it costs 537 ms** on a machine with Claude Code
    /// and Codex both installed and signed in (`the_cost_of_asking.rs`). Four surfaces ask
    /// independently — startup, the character editor, the connections panel, a dialogue — so a
    /// pass through the Launcher spent over two seconds spawning processes to learn the same
    /// answer four times.
    ///
    /// **The reading is kept, never invented.** `fresh` re-measures, which is what the panels'
    /// own button does; signing somebody in throws it away, because that is the one action that
    /// makes it wrong. Nothing expires it on a timer: a timer would be a guess about how often
    /// somebody installs a CLI, and the honest alternative is a button that says what it does.
    pub fn agents(&self, fresh: bool) -> Vec<epoch_engine::agent::AgentStatus> {
        let known = if !fresh {
            guard(&self.agents_seen).clone()
        } else {
            None
        };
        let measured = known.unwrap_or_else(|| {
            let measured = agents_here().survey();
            *guard(&self.agents_seen) = Some(measured.clone());
            measured
        });
        self.with_refusals(measured)
    }

    /// Let a failed turn overrule what the program said about itself.
    ///
    /// `claude auth status --json` answers `"loggedIn": true` with an expired token, and every
    /// turn then dies with somebody else's 401. A reading that a real turn has contradicted must
    /// not keep claiming the thing works — the turn is the evidence the reading was standing in
    /// for.
    pub(crate) fn with_refusals(
        &self,
        mut measured: Vec<epoch_engine::agent::AgentStatus>,
    ) -> Vec<epoch_engine::agent::AgentStatus> {
        let refused = guard(&self.agent_refused).clone();
        for status in &mut measured {
            if let Some(why) = refused.get(&status.id) {
                status.signed_in = Some(false);
                status.note = Some(why.clone());
            }
        }
        measured
    }

    /// A turn came back saying the credential is no good.
    ///
    /// Only for what is unmistakably about authentication. A model refusing a request, a network
    /// that dropped, a project folder that is missing — none of those say anything about being
    /// signed in, and marking an agent signed-out over one would be an invented reading in the
    /// other direction.
    pub(crate) fn refused_by(&self, agent: &str, why: &str) {
        let lower = why.to_lowercase();
        let about_the_credential = lower.contains("401")
            || lower.contains("oauth")
            || lower.contains("authenticate")
            || lower.contains("unauthorized")
            || lower.contains("invalid api key")
            || lower.contains("credit balance");
        if !about_the_credential {
            return;
        }
        guard(&self.agent_refused).insert(
            agent.to_owned(),
            // Its own words. Epoch has no idea what expired or when, and inventing a friendlier
            // sentence would lose the only detail that says what to do.
            format!("Its last turn was refused: {}", why.trim()),
        );
    }

    /// A turn worked, so whatever it said before is no longer true.
    pub(crate) fn accepted_by(&self, agent: &str) {
        guard(&self.agent_refused).remove(agent);
    }

    /// Forget what was measured about the agents, so the next ask runs the programs again.
    pub(crate) fn forget_agents(&self) {
        *guard(&self.agents_seen) = None;
    }

    /// Open an agent's own sign-in, in its own window.
    ///
    /// Epoch starts the flow and stops there. It shows no password field, reads nothing back and
    /// stores no token — the credential is between the user and the agent, and the only thing
    /// Epoch removes is having to know a terminal was needed.
    ///
    /// Returns when the window is open, not when the sign-in worked. Whether it worked is
    /// [`Self::agents`]'s answer afterwards, which is a measurement rather than a promise.
    pub fn sign_in_agent(&self, agent_id: &str) -> Result<(), String> {
        // Whatever was measured about this agent is about to stop being true, and a signed-out
        // reading kept after a sign-in is the one stale answer that reads as a bug.
        self.forget_agents();
        // A sign-in is somebody answering the thing a refusal was about. Whether it worked is
        // what asking again says — keeping the old refusal would make a fixed sign-in look
        // broken until a turn happened to prove otherwise.
        guard(&self.agent_refused).clear();
        let agents = agents_here();
        let agent = agents
            .get(agent_id)
            .ok_or_else(|| format!("'{agent_id}' is not an agent this build knows"))?;
        agent.sign_in().map_err(|why| why.to_string())
    }

    /// The reasoning shortcut for one character in the active Quest.
    pub fn reasoning_dial(
        &self,
        character_id: &str,
    ) -> Result<epoch_engine::deliberation::Dial, String> {
        let id = CharacterId::new(character_id)?;
        let inner = self.lock();
        let definition = inner
            .registry
            .character(&id)
            .ok_or_else(|| format!("no character called '{id}'"))?;
        let mut dial = epoch_engine::deliberation::dial(definition);
        if let Some(world) = inner.world_id.as_deref() {
            if let Some(reasoning) = inner
                .quests
                .active(world)
                .and_then(|quest| quest.session.reasoning)
                .or(inner.prepared_session.reasoning)
            {
                dial.chosen = Some(reasoning.as_str());
            }
        }
        Ok(dial)
    }

    /// Move the active Quest along this character's measured ladder.
    ///
    /// `None` removes the Quest override and returns to the character's authored default.
    pub fn choose_reasoning(&self, character_id: &str, rung: Option<String>) -> Result<(), String> {
        let id = CharacterId::new(character_id)?;
        let mut inner = self.lock();
        let definition = inner
            .registry
            .character(&id)
            .ok_or_else(|| format!("no character called '{id}'"))?;
        let dial = epoch_engine::deliberation::dial(definition);
        let chosen = match rung.as_deref() {
            None => None,
            Some(raw) => {
                if !dial.rungs.iter().any(|candidate| candidate.id == raw) {
                    return Err(format!(
                        "{character_id} cannot be asked for {raw} deliberation"
                    ));
                }
                Some(
                    epoch_kernel::Reasoning::from_id(raw)
                        .ok_or_else(|| format!("'{raw}' is not a level this build knows"))?,
                )
            }
        };
        let world = inner
            .world_id
            .clone()
            .ok_or_else(|| "no World is open".to_string())?;
        if let Some(quest) = inner.quests.active_mut(&world) {
            quest.session.reasoning = chosen;
            inner
                .quests
                .save_active(&vault_dir(), &world)
                .map_err(|err| err.to_string())
        } else {
            inner.prepared_session.reasoning = chosen;
            Ok(())
        }
    }

    /// Which kind of brain this character has, so a caller can pick the right turn.
    ///
    /// Returned rather than inferred from a failure: dispatching on "the provider lookup threw"
    /// would make a missing backend indistinguishable from an agent.
    pub fn brain_of(&self, character_id: &str) -> Option<&'static str> {
        let id = CharacterId::new(character_id).ok()?;
        let inner = self.lock();
        match &inner.registry.character(&id)?.mind.as_ref()?.brain {
            epoch_kernel::Brain::Model { .. } => Some("model"),
            epoch_kernel::Brain::Agent { .. } => Some("agent"),
        }
    }

    /// Add a second (or third) sign-in of one agent program.
    ///
    /// ## What Epoch does, and what it deliberately does not
    ///
    /// It generates an id nothing else uses, records the **label the user typed**, and creates
    /// the directory that sign-in will live in. Then it starts the agent's own login, in the
    /// agent's own window, with the agent's own environment variable pointing at that directory —
    /// and steps back. Epoch shows no password field, reads nothing back and stores no token.
    ///
    /// The label is the user's word and is never checked against anything. Claude Code will tell
    /// Epoch which email is signed in and that is shown beside the label; Codex cannot be asked
    /// at all, so for that one the label is the only name there is. Inventing an address would be
    /// the invented gauge the Launcher forbids.
    pub fn add_agent_account(&self, kind: &str, label: &str) -> Result<String, String> {
        let label = label.trim();
        if label.is_empty() {
            return Err("give this account a name you will recognise — `work`, `personal`.".into());
        }
        if epoch_engine::agents::name_of(kind).is_none() {
            return Err(format!("'{kind}' is not an agent this build knows"));
        }
        // **Only where a second sign-in is a real thing.** Gemini CLI signs in with an API key
        // and cannot be asked whether that key still works, so two rows of it would be two things
        // nobody could tell apart.
        if kind == epoch_engine::agents::gemini::ID {
            return Err(
                "Gemini CLI signs in with an API key and has no way to be asked which account that is, so Epoch cannot tell two of them apart."
                    .into(),
            );
        }

        let vault = vault_dir();
        let mut settings = Settings::load(&vault);
        let id = settings.next_account_id(&vault, kind);
        // The directory before the login, because the login is what fills it.
        std::fs::create_dir_all(epoch_engine::settings::account_home(&vault, &id))
            .map_err(|why| format!("that account's folder could not be made: {why}"))?;
        settings
            .agent_accounts
            .push(epoch_engine::settings::AgentAccount {
                kind: kind.to_owned(),
                id: id.clone(),
                label: label.to_owned(),
            });
        settings.save(&vault).map_err(|e| e.to_string())?;
        self.lock().settings = settings;
        // A new account has never been probed, and the kept survey would answer for the old list.
        *guard(&self.agents_seen) = None;
        Ok(id)
    }

    /// Rename one added account. Its id never changes.
    ///
    /// **Because a character's brain names the id.** A label is something somebody fixes on a
    /// Tuesday; if renaming moved the id it would silently take that character's brain away.
    pub fn rename_agent_account(&self, id: &str, label: &str) -> Result<(), String> {
        let label = label.trim();
        if label.is_empty() {
            return Err("a name cannot be empty".into());
        }
        let vault = vault_dir();
        let mut settings = Settings::load(&vault);
        let found = settings
            .agent_accounts
            .iter_mut()
            .find(|it| it.id == id)
            .ok_or_else(|| format!("'{id}' is not an account this machine added"))?;
        found.label = label.to_owned();
        settings.save(&vault).map_err(|e| e.to_string())?;
        self.lock().settings = settings;
        *guard(&self.agents_seen) = None;
        Ok(())
    }

    /// Forget an added account.
    ///
    /// Epoch removes it from its own list and **leaves the agent's own directory alone**. The
    /// sign-in inside it belongs to the agent, and deleting somebody's credentials because they
    /// tidied a row is not a thing a surface does quietly. Said in the answer, so the user can go
    /// and remove it themselves if that is what they meant.
    pub fn remove_agent_account(&self, id: &str) -> Result<String, String> {
        let vault = vault_dir();
        let mut settings = Settings::load(&vault);
        let before = settings.agent_accounts.len();
        settings.agent_accounts.retain(|it| it.id != id);
        if settings.agent_accounts.len() == before {
            return Err(format!("'{id}' is not an account this machine added"));
        }
        settings.save(&vault).map_err(|e| e.to_string())?;
        self.lock().settings = settings;
        *guard(&self.agents_seen) = None;
        Ok(format!(
            "Removed from Epoch. Its sign-in is still in {} — Epoch does not delete somebody's credentials for them.",
            epoch_engine::settings::account_home(&vault, id).display()
        ))
    }

    /// Swap the live registry for one built from this configuration.
    /// The roster changed, so what can think changed with it.
    ///
    /// **The registry is built once and held**, which is right — a Provider carries a bearer and
    /// reading the store per request would be worse in every way. But nothing rebuilt it when a
    /// machine was paired or unpaired, so the live registry kept whichever Bridges existed at
    /// startup: a machine paired a moment ago could not be reached until Epoch was restarted,
    /// and one that had been unpaired went on being listed and asked.
    ///
    /// Probing did not help and could not: `list_providers` surveys this same registry, so it
    /// was faithfully reporting a roster that no longer existed.
    ///
    /// Called by every path that writes `Pairings` — enrolling, pairing, unpairing and changing
    /// grants. A grant is in the list because [`Backends::registry`] only builds Bridges that
    /// may compute, so taking that grant away has to take the Provider away too.
    pub(crate) fn roster_changed(&self) {
        self.rebuild_backends(&epoch_engine::backends::Backends::load(&vault_dir()));
    }

    /// The knobs one Provider has for one model.
    ///
    /// Asked per model rather than per Provider because the interesting bound — how much
    /// context there is — belongs to the model (ADR-0026). Reaches the network, so it is its
    /// own command and the panel renders without waiting for it.
    ///
    /// An unknown Provider gets an empty list rather than an error: a character authored
    /// against a backend this build does not have is a file to keep working with, not a
    /// failure to report at somebody who cannot act on it.
    pub fn surface(&self, provider: &str, model: &str) -> epoch_engine::Surface {
        self.providers
            .read()
            .expect("providers lock")
            .get(provider)
            .map(|p| p.surface(model))
            .unwrap_or_default()
    }

    /// How much this model says it can hold, remembered after the first ask.
    ///
    /// `None` when the backend will not say — **unknown, never a guess**. A hosted Provider
    /// that reports nothing is answering honestly, and turning that into a number here would
    /// make a constant look like a measurement two layers further up.
    pub(crate) fn measured_window(&self, provider: &str, model: &str) -> Option<u32> {
        self.reported(provider, model).window
    }

    /// What this model says it can do, remembered with the rest of its answer.
    ///
    /// `None` means the backend would not say (see [`epoch_engine::provider::Declared`]) — which
    /// is *unasked*, never "cannot".
    pub(crate) fn declared(
        &self,
        provider: &str,
        model: &str,
    ) -> Option<epoch_engine::provider::Declared> {
        self.reported(provider, model).can
    }

    /// One ask per model, remembered whole — and **never asked on the caller's thread**.
    ///
    /// Both callers run inside a turn, holding the World's lock. Asking a backend from there
    /// meant a network round trip with the whole World locked, and Ollama answers `/api/show`
    /// only after it has finished whatever it is doing — including loading a 12 GB model,
    /// which Epoch itself asks for on every turn while the crew is not resident. Measured: the
    /// window stopped responding for as long as the load took, every turn, and the World is
    /// what stops responding, not one panel.
    ///
    /// So a cold model answers `Surface::default()` — *unknown* — and the measurement happens
    /// on its own thread for the next turn. That is the answer the code below already handles
    /// and already documents: "Unknown stays unknown, and a conservative budget is the right
    /// answer to not knowing." Nothing is invented, nothing waits.
    pub(crate) fn reported(&self, provider: &str, model: &str) -> epoch_engine::Surface {
        let key = format!("{provider}/{model}");
        if let Some(known) = guard(&self.reported).get(&key) {
            return known.clone();
        }
        self.measure_soon(provider, model);
        epoch_engine::Surface::default()
    }

    /// Ask a backend what one model looks like, off this thread, and remember the answer.
    ///
    /// At most one flight per model: the key is claimed before the thread starts, so a
    /// character answering three turns while the first ask is still out does not queue three
    /// identical requests behind the same busy backend.
    pub(crate) fn measure_soon(&self, provider: &str, model: &str) {
        let key = format!("{provider}/{model}");
        {
            let mut flights = guard(&self.measuring);
            if !flights.insert(key.clone()) {
                return;
            }
        }
        let provider = provider.to_owned();
        let model = model.to_owned();
        let providers = Arc::clone(&self.providers);
        let reported = Arc::clone(&self.reported);
        let measuring = Arc::clone(&self.measuring);
        std::thread::spawn(move || {
            let asked = providers
                .read()
                .expect("providers lock")
                .get(&provider)
                .map(|p| p.surface(&model))
                .unwrap_or_default();
            /*
                **An answer with nothing in it is not an answer, and must not be kept.**

                A surface that says nothing — no window, no capabilities — is what a backend
                returns when it could not be reached, or when the model is cold and the server
                publishes its window only while it is resident. Caching that turned a moment
                into a permanent fact: every later turn read the empty answer, never asked
                again, and composed against the conservative default for the life of the
                session.

                So it is remembered when it says something and left unknown when it does not,
                which costs one cheap listing on the next turn and is what `reported`’s own
                comment already promises: *the measurement happens on its own thread for the
                next turn*.
            */
            if asked == epoch_engine::Surface::default() {
                guard(&measuring).remove(&key);
                return;
            }
            reported
                .lock()
                .expect("reported lock")
                .insert(key.clone(), asked);
            guard(&measuring).remove(&key);
        });
    }

    /// Change what the bridge calls the orchestrator. Blank returns them to unnamed.
    pub fn rename_orchestrator(&self, name: &str) -> Result<(), String> {
        Orchestrator::rename(&vault_dir(), name).map_err(|err| err.to_string())
    }

    /// Give the orchestrator a face, or take it away.
    pub fn set_orchestrator_portrait(&self, image: Option<&str>) -> Result<(), String> {
        let vault = vault_dir();
        match image {
            Some(data) => Orchestrator::set_portrait(&vault, data),
            None => Orchestrator::clear_portrait(&vault),
        }
        .map_err(|err| err.to_string())
    }

    /// Give a character a different brain, and record that the user did it.
    ///
    /// ## Why this exists rather than a failover
    ///
    /// The Runtime never chooses a Brain. Canonical parameters are behavioural identity
    /// (ADR-0026) and the model is the same argument one field up: the same prompt on Gemma,
    /// Qwen and Claude reasons differently, so a silent substitution is a character behaving
    /// differently with nobody told. When the assigned machine cannot be reached, Epoch says so
    /// and offers the choice — and this is the user taking it.
    ///
    /// ## Why it writes to the Chronicle
    ///
    /// A Chronicle holding only the replies would show one person changing their mind, when
    /// what happened is that somebody swapped who was thinking. Handoffs have visible causes
    /// (ADR-0025), and `because` carries the question that was being answered.
    ///
    /// ## What it refuses
    ///
    /// A backend this machine does not offer, through the same list the editor validates
    /// against — a question asked in a hurry must not be able to write something a considered
    /// edit would have refused.
    pub fn reassign_brain(
        &self,
        character_id: &str,
        backend: &str,
        model: &str,
        because: &str,
    ) -> Result<(), String> {
        let id = CharacterId::new(character_id)?;
        let backend = backend.trim();
        let model = model.trim();
        if model.is_empty() {
            return Err("a Service needs a model to think with".to_owned());
        }
        if !self.backends().offers(backend) {
            return Err(format!("'{backend}' is not a backend this machine offers"));
        }

        let mut inner = self.lock();
        let definition = inner
            .registry
            .character(&id)
            .cloned()
            .ok_or_else(|| format!("no character called '{character_id}'"))?;

        // What they had, in the words the user was just reading. Taken before the write, or the
        // record would say the same thing twice.
        let from = definition
            .mind
            .as_ref()
            .map(|mind| {
                format!(
                    "{} · {}",
                    mind.brain.provider().unwrap_or("an agent"),
                    mind.model()
                )
            })
            .unwrap_or_else(|| "nothing assigned".to_owned());
        let to = format!("{backend} · {model}");

        // Parameters and tuning are carried, never rebuilt: answering a question about *where*
        // somebody thinks must not quietly change *how* they think.
        let mut mind = definition
            .mind
            .clone()
            .unwrap_or_else(|| epoch_kernel::Mind::new(backend, model));
        mind.brain = epoch_kernel::Brain::Model {
            provider: backend.to_owned(),
            model: model.to_owned(),
        };
        // Saved whole, through the one writer every edit uses. Everything else about them is
        // carried across untouched: the question that was asked was *where*, not *who*.
        inner
            .registry
            .save(epoch_kernel::CharacterDefinition {
                mind: Some(mind),
                ..definition
            })
            .map_err(|err| err.to_string())?;

        let world = inner.world_id.clone();
        if let Some(world) = world {
            let at = epoch_engine::now_ms();
            if let Some(quest) = inner.quests.active_mut(&world) {
                quest.record(
                    at,
                    epoch_kernel::Entry::Reassigned {
                        character: id.clone(),
                        from,
                        to,
                        because: because.trim().to_owned(),
                    },
                );
            }
            inner.remember_quests();
        }

        inner.note_problems();
        inner.repopulate();
        Ok(())
    }

    /// Save an edit to one member of the crew.
    ///
    /// Applies to the vault, then repopulates the open World if there is one — so an edit
    /// made in the Launcher is already true the next time the World is looked at.
    pub fn save_character(&self, edit: CharacterEdit) -> Result<(), String> {
        // What the assigned Provider says it has, asked before the lock: `controls` can reach
        // the network to measure a model's real context limit, and holding the world's mutex
        // across that would freeze every surface reading it.
        //
        // An unknown or unassigned Provider yields none, and `apply` then refuses any tuning
        // the form sent — which is right: a value nobody can validate is a value nobody should
        // write into somebody's character.
        let controls = match (&edit.provider, &edit.model) {
            (Some(provider), Some(model)) => self.surface(provider, model),
            _ => epoch_engine::Surface::default(),
        };
        let machine = epoch_engine::launcher::Machine {
            // Asked for, never rebuilt. A list assembled here from `.backends` alone is how a
            // paired Mac stayed unsaveable after `offers` had already learned about Bridges.
            offers: self.backends().offered(),
            window: controls.window,
            controls: controls.controls,
            // Measured now, not remembered. Somebody can install Claude Code while Epoch is
            // open, and the first edit after that should be able to choose it.
            agents: agents_here()
                .survey()
                .into_iter()
                .filter(|a| a.installed)
                .map(|a| a.id)
                .collect(),
        };

        let mut inner = self.lock();
        edit.apply(&mut inner.registry, &machine)?;
        inner.note_problems();
        inner.repopulate();
        Ok(())
    }

    /// Make somebody who did not exist, and return who they are.
    ///
    /// The crew starts at zero and stays there until the user makes the first person themselves
    /// (ADR-0023). Nothing here decides who they are: no model, no face, no home.
    pub fn hire_character(
        &self,
        name: &str,
        archetype: &str,
        role: &str,
        prompt: &str,
    ) -> Result<String, String> {
        let archetype = epoch_kernel::CharacterArchetype::from_id(archetype)
            .ok_or_else(|| format!("'{archetype}' is not an archetype this build knows"))?;

        let mut inner = self.lock();
        let id = epoch_engine::hiring::hire(&mut inner.registry, name, archetype, role, prompt)?;
        inner.note_problems();
        inner.repopulate();
        Ok(id.to_string())
    }

    /// Give a character one of their drawings, or take it away.
    ///
    /// Slots are deliberately independent: the picture that walks around the World, the face
    /// that identifies them in a conversation, and a sheet for each action answer different
    /// questions, and improving one should never be able to ruin another.
    ///
    /// The slot is parsed here rather than trusted — a word this build does not know is refused
    /// at the door, like every other choice arriving from a surface.
    pub fn set_character_art(
        &self,
        character_id: &str,
        slot: &str,
        image: Option<&str>,
        cut: Option<epoch_kernel::Sheet>,
    ) -> Result<(), String> {
        let id = CharacterId::new(character_id)?;
        let slot = epoch_engine::ArtSlot::from_id(slot)
            .ok_or_else(|| format!("'{slot}' is not a drawing a character has"))?;
        let mut inner = self.lock();
        inner
            .registry
            .set_art(&id, slot, image, cut)
            .map_err(|err| err.to_string())?;
        inner.note_problems();
        inner.repopulate();
        Ok(())
    }

    /// Change how an action's sheet is read, leaving the sheet alone.
    ///
    /// What the editor does while somebody watches the preview and adjusts the grid.
    pub fn set_character_cut(
        &self,
        character_id: &str,
        action: &str,
        cut: epoch_kernel::Sheet,
    ) -> Result<(), String> {
        let id = CharacterId::new(character_id)?;
        let action = epoch_kernel::Action::from_id(action)
            .ok_or_else(|| format!("'{action}' is not something a character does"))?;
        let mut inner = self.lock();
        inner
            .registry
            .set_cut(&id, action, cut)
            .map_err(|err| err.to_string())?;
        inner.note_problems();
        inner.repopulate();
        Ok(())
    }

    /// **What removing a character would do**, so the user can accept it before it happens.
    ///
    /// The Engine names the files; this joins in the one thing it cannot see — how much of this
    /// World's History has them in it. Read from digests rather than whole Quests, because a
    /// confirmation dialogue must not spend a second reading transcripts nobody will look at.
    pub fn character_removal(&self, character_id: &str) -> Result<RemovalView, String> {
        let id = CharacterId::new(character_id)?;
        let inner = self.lock();
        let mut plan = inner
            .registry
            .removal_of(&id)
            .ok_or_else(|| format!("there is nobody called '{character_id}'"))?;

        if let Some(world) = inner.world_id.clone() {
            drop(inner);
            let (digests, _) = epoch_engine::QuestDigest::read_all(&vault_dir(), &world);
            let theirs = digests
                .iter()
                .filter(|quest| quest.participants().contains(&id))
                .count();
            if theirs > 0 {
                plan.survives.insert(
                    0,
                    format!(
                        "{theirs} {} in this World {} them, and {} untouched.",
                        if theirs == 1 {
                            "conversation"
                        } else {
                            "conversations"
                        },
                        if theirs == 1 { "names" } else { "name" },
                        if theirs == 1 { "stays" } else { "stay" }
                    ),
                );
            }
        }

        Ok(RemovalView::of(&plan))
    }

    /// Remove a character, after the user has accepted what it does.
    ///
    /// The plan is rebuilt here rather than carried from the surface: a list of paths that came
    /// back from a window is a list of paths somebody could change, and this deletes files.
    pub fn remove_character(&self, character_id: &str) -> Result<Vec<String>, String> {
        let id = CharacterId::new(character_id)?;
        let mut inner = self.lock();
        let problems = inner.registry.remove(&id).map_err(|err| err.to_string())?;
        inner.note_problems();
        inner.repopulate();
        Ok(problems)
    }
}

// ---------------------------------------------------------------------------
// Keeping a brain on the card
// ---------------------------------------------------------------------------

/// What the lamp above one character's conversation reads.
///
/// **Two facts, deliberately not one.** The switch is what the user asked for; the lamp is what
/// is actually on the graphics card. A lamp wired to the switch would say green while
/// llama.cpp's router — which holds one model at a time — had quietly evicted this one for
/// somebody else's turn, and green again for a model this World has never loaded but LM Studio
/// has open in its own window. They usually agree. When they do not, the card is right.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Warmth {
    /// Which model this is about, in the backend's own words.
    pub model: String,
    /// The **intent**: whether this character's brain is being held after it answers.
    ///
    /// Resolved, so it already accounts for the machine-wide `concurrentCrew` default that
    /// governs anybody the user has not made a choice about.
    pub holding: bool,
    /// Whether the user chose this themselves, as opposed to inheriting the default.
    ///
    /// The surface says which it is. A control showing a value nobody set, with nothing marking
    /// it as a default, is how somebody comes to believe they chose it.
    pub chosen: bool,
    /// The **reading**: in memory right now, measured from the server that holds it.
    ///
    /// `None` is *unasked*, never "no" — a hosted brain has no memory of the user's to report,
    /// an agent has no model at all, and a local server that will not answer has said nothing
    /// about what it holds. The surface draws no lamp rather than a cold one, because a lamp
    /// that cannot read is worse than no lamp.
    pub resident: Option<bool>,
}

impl World {
    /// What is on the card for one character, and what the user asked for.
    ///
    /// **Reaches the network, and never while holding the World's lock.** The lock is taken to
    /// read who this character's brain is and released before anything is asked — the same
    /// mistake `reported` documents, which froze the whole window for as long as a 12 GB load
    /// took.
    pub fn warmth(&self, character_id: &str) -> Result<Warmth, String> {
        let id = CharacterId::new(character_id)?;
        let (provider, model, holding, chosen) = {
            let inner = self.lock();
            let mind = inner
                .registry
                .character(&id)
                .ok_or_else(|| format!("no character called '{id}'"))?
                .mind
                .clone()
                // An agent brain is not a model on this machine. Nothing to hold, nothing to
                // read, and saying so is better than a control that governs nothing.
                .ok_or_else(|| format!("{id} has no brain assigned"))?;
            (
                mind.provider().map(str::to_owned),
                mind.model().to_owned(),
                inner.keep_loaded_for(&id) != epoch_engine::KeepLoaded::Never,
                inner.held.contains_key(&id),
            )
        };

        let resident = provider.as_ref().and_then(|backend| {
            self.providers
                .read()
                .expect("providers lock")
                .get(backend)
                .and_then(|p| p.resident(&model))
        });

        Ok(Warmth {
            model,
            holding,
            chosen,
            resident,
        })
    }

    /// Hold this character's brain on the card, or let it go now.
    ///
    /// Turning it **off releases immediately** rather than waiting for the next turn to end.
    /// The button says *unload*, so it unloads — a switch whose effect arrives whenever
    /// something else happens next is the kind of control people press twice.
    ///
    /// Turning it **on loads nothing**. Reading a model onto the card takes seconds and gigabytes
    /// and there is nothing to say with it yet; the next turn loads it and it stays. So the lamp
    /// goes green when the model is genuinely there, which is the only thing it is allowed to
    /// mean.
    pub fn hold_warm(&self, character_id: &str, keep: bool) -> Result<Warmth, String> {
        let id = CharacterId::new(character_id)?;
        let brain = {
            let mut inner = self.lock();
            inner.held.insert(id.clone(), keep);
            inner
                .registry
                .character(&id)
                .and_then(|c| c.mind.clone())
                .and_then(|m| Some((m.provider()?.to_owned(), m.model().to_owned())))
        };

        if !keep {
            if let Some((backend, model)) = brain {
                // Best-effort, like every other release: failing to free memory is not worth
                // failing on, and the lamp reads the card afterwards either way.
                if let Some(provider) = self.providers.read().expect("providers lock").get(&backend)
                {
                    provider.release(&model);
                }
            }
        }

        self.warmth(character_id)
    }
}
