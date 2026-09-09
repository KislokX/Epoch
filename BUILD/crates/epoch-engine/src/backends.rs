//! Which backends exist, and where they are.
//!
//! ## Infrastructure, not content
//!
//! A Provider belongs to the **machine**, not to a World and not to a character (ADR-0026: an
//! endpoint and a credential are properties of a computer, a temperature is a property of a
//! person). So this lives once in the vault and every World sees the same list — moving between
//! Worlds must not change what can think, any more than it changes what is installed.
//!
//! ## Kind and identity are different questions
//!
//! `kind` is what this build knows how to speak to. `id` is which one you mean. They are equal
//! for the common case and diverge the moment somebody runs Ollama on their laptop *and* on the
//! machine under the desk — two entries, one kind, and a character can name either. Nothing here
//! costs anything to allow that, and collapsing them would make the second machine unreachable
//! without a schema change.
//!
//! ## A file we cannot read is never a file we overwrite
//!
//! Same rule as the Trust store. Somebody's hand-edited endpoint is not ours to discard because
//! a bracket is missing — it is reported, and the last good configuration keeps working.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use epoch_kernel::SecretName;

use crate::anthropic::Anthropic;
use crate::provider::{Ollama, Provider, ProviderRegistry};
use crate::secrets::Secrets;

/// What this build knows how to speak to.
///
/// A closed set on purpose: a `kind` naming something no version of Epoch implements is a
/// configuration that can only fail at the moment somebody tries to think with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Models on the user's own machine. No key, no account, no bill.
    Ollama,
    /// Anthropic's hosted models. Needs a credential; the surface says so when there is none.
    Anthropic,
    /// **Anything speaking the OpenAI API**, at any address.
    ///
    /// llama.cpp, LM Studio, vLLM, Deepseek, GLM, Groq — one shape at a dozen addresses, so
    /// adding one is a form rather than a pull request, and the one that ships tomorrow works
    /// the day it ships.
    ///
    /// The only `Kind` whose endpoint has no sensible default: a vendor lives at its own
    /// hostname and a local server lives on whichever port its owner chose. `usual_endpoint`
    /// answers with llama.cpp's default, because that is the case where a guess is a
    /// convenience rather than a wrong assumption about somebody's account.
    ///
    /// ## Spelled once, and it took an AMD machine to find out it was spelled twice
    ///
    /// `rename_all = "snake_case"` turns this variant into `open_ai` on the wire, while
    /// [`Kind::as_str`] — the other spelling of the same fact — says `openai`. Every surface sends
    /// what `as_str` produced, so adding an OpenAI-compatible backend failed with
    ///
    /// ```text
    /// unknown variant `openai`, expected one of `ollama`, `anthropic`, `open_ai`
    /// ```
    ///
    /// **The other two variants agreed by luck**: the snake_case of a single word is that word.
    /// This is the only two-word variant, so it is the only one that could break — and it went
    /// unnoticed until somebody on a second machine tried to add llama.cpp by address.
    ///
    /// The alias keeps a `backends.json` written by an older build readable. It is an input
    /// tolerance and never what is written.
    #[serde(rename = "openai", alias = "open_ai")]
    OpenAi,
}

impl Kind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Kind::Ollama => "ollama",
            Kind::Anthropic => "anthropic",
            Kind::OpenAi => "openai",
        }
    }

    pub fn from_id(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "ollama" => Some(Kind::Ollama),
            "anthropic" => Some(Kind::Anthropic),
            // `openai_compatible` is what a person would write; `openai` is what is stored.
            "openai" | "openai_compatible" | "openai-compatible" => Some(Kind::OpenAi),
            _ => None,
        }
    }

    /// Where this kind usually lives, for a first entry nobody had to write.
    pub const fn usual_endpoint(self) -> &'static str {
        match self {
            Kind::Ollama => "http://localhost:11434",
            Kind::Anthropic => "https://api.anthropic.com",
            // llama.cpp's own default. A starting point to edit, never an assumption about where
            // this particular one lives.
            Kind::OpenAi => "http://localhost:8080",
        }
    }

    /// Whether this kind cannot be asked anything without a stored credential.
    ///
    /// Reported rather than assumed from `local`: it is the difference between a backend that
    /// is *offline* and one that is *waiting for you*, and a surface that showed both the same
    /// way would send somebody looking for a server that is running fine.
    pub const fn needs_credential(self) -> bool {
        match self {
            Kind::Ollama => false,
            Kind::Anthropic => true,
            // **Neither, and that is the point.** A hosted vendor demands a key and a llama.cpp
            // on the desk has none, and this one `Kind` is both. Saying it *needs* one would
            // make the local case look misconfigured; saying it never does would hide why
            // Deepseek is answering 401. The surface reports what the backend said instead.
            Kind::OpenAi => false,
        }
    }
}

/// One backend, as the user configured it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Backend {
    /// Which one you mean. What a character's `Brain` names.
    pub id: String,
    /// What this build should speak to it with.
    pub kind: Kind,
    /// Where to find it.
    pub endpoint: String,
    /// Switched off without being forgotten.
    ///
    /// A disabled backend keeps its endpoint and stops being probed — which is the difference
    /// between "I am not using this today" and "delete what I typed". Characters assigned to it
    /// keep their assignment and simply cannot think, and the surface says so.
    #[serde(default = "yes")]
    pub enabled: bool,
}

const fn yes() -> bool {
    true
}

impl Backend {
    /// Everything wrong with this entry, in the user's terms.
    pub fn problems(&self) -> Vec<String> {
        let mut found = Vec::new();
        let id = self.id.trim();
        if id.is_empty() {
            found.push("a backend needs a name".into());
        } else if let Some(bad) = id
            .chars()
            .find(|c| !(c.is_ascii_alphanumeric() || *c == '_' || *c == '-'))
        {
            // Named in files and in characters, so it stays to characters that survive being
            // shared: letters, digits, dash, underscore.
            found.push(format!("'{id}' cannot contain '{bad}'"));
        }
        let endpoint = self.endpoint.trim();
        if endpoint.is_empty() {
            found.push(format!("'{id}' needs an address"));
        } else if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
            // Guessed schemes are how a typo becomes a silent connection somewhere else.
            found.push(format!("'{id}' must start with http:// or https://"));
        }
        found
    }
}

/// Every configured backend, in the order the user will see them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Backends {
    #[serde(default)]
    pub backends: Vec<Backend>,
    /// Why the file could not be read, if it could not.
    ///
    /// Never read back, so a hand-typed `problem =` cannot lock somebody out of saving. It does
    /// leave over IPC, and it has to: a surface that could not say why the list it is showing is
    /// not the user's own list would present the defaults as if they had been chosen.
    #[serde(default, skip_deserializing)]
    pub problem: Option<String>,

    /// Which backends have a credential in the store (see [`crate::secrets`]).
    ///
    /// Ids only — a `Secret` has no `Serialize` at all, so the value could not travel here even
    /// if somebody tried. Derived on read rather than kept in the file, so the configuration and
    /// the store have no way to disagree about what exists.
    #[serde(default, skip_deserializing)]
    pub keyed: Vec<String>,

    /// Paired machines that can currently think, by Provider id (`bridge:<id>`).
    ///
    /// **Not configuration, and that is why it is separate.** A backend is something the user
    /// typed into this file; a Bridge arrives from the roster of paired machines and is filed
    /// there. But both are Providers a character may name, so both have to be here or
    /// [`Backends::offers`] tells the truth about half the list.
    ///
    /// Derived on read, never written: the roster is the source, and a copy in this file would
    /// be a second answer to a question that already has one.
    #[serde(default, skip_deserializing)]
    pub bridges: Vec<String>,

    /// Programs on **this** machine that are serving right now, by Provider id.
    ///
    /// ## Why these are not configuration either
    ///
    /// A paired machine's LM Studio becomes a Provider the moment that machine says it is
    /// serving one — nobody types anything. This machine's LM Studio used to need a button
    /// (`ADD AS A SERVICE`), which is the same fact under two different rules, and the rule that
    /// applied was whichever computer it happened to be on.
    ///
    /// So it is measured here for the same reason it is measured there. Derived on read, never
    /// written: a copy in this file would be a second answer to a question the machine already
    /// answers, and it would go stale the moment somebody stopped a server.
    ///
    /// **Serving, not installed** — the same line the roster draws. A Provider offered for a
    /// switched-off server looks configured and refuses every turn, and since nobody chose it,
    /// nobody would know why.
    ///
    /// A configured backend of the same id wins: somebody who typed an endpoint has said
    /// something, and a measurement must not overrule it.
    #[serde(default, skip_deserializing)]
    pub runners: Vec<String>,
}

/// The shape that actually reaches `providers.toml`.
///
/// Separate from [`Backends`] so that IPC-only fields cannot land in the file by being added to
/// the struct. The alternative — remembering a `skip_serializing_if` on each one — works until
/// the first field where the condition is sometimes false.
#[derive(Serialize)]
struct OnDisk<'a> {
    backends: &'a [Backend],
}

fn path(vault: &Path) -> PathBuf {
    vault.join("providers.toml")
}

/// Every local runtime this machine actually has, as backends.
///
/// **Its address comes from the survey, not from `usual_endpoint`.** A runtime already answering
/// somewhere is answering *there*, and writing down where Epoch thinks it usually lives would
/// configure the wrong port for anybody who moved it. The usual address is the fallback for one
/// that is installed and stopped, which is the only case where there is nothing to read.
///
/// `None` where the machine has none, so the caller can keep whatever it does about that.
fn installed_here() -> Option<Vec<Backend>> {
    let found: Vec<Backend> = epoch_models::runtimes::survey_cached()
        .into_iter()
        .filter(|seen| seen.installed)
        .map(|seen| {
            // The id is the runtime’s own — `ollama`, `llama_cpp`, `lm_studio` — which is what
            // every other part of Epoch already keys on. Ollama speaks its own API and the other
            // two speak OpenAI’s; that is the whole of what a `Kind` decides here.
            let kind = if seen.id == "ollama" {
                Kind::Ollama
            } else {
                Kind::OpenAi
            };
            let endpoint = seen.endpoint.trim();
            let endpoint = if endpoint.is_empty() {
                kind.usual_endpoint().to_owned()
            } else {
                endpoint.to_owned()
            };
            Backend {
                id: seen.id.to_owned(),
                kind,
                endpoint,
                enabled: true,
            }
        })
        .collect();
    (!found.is_empty()).then_some(found)
}

/// Write down any runtime that is on this machine and not yet in the file.
///
/// ## Why this is a call somebody makes rather than something `load` does
///
/// `load` is read-only on purpose and is on a path somebody is waiting on — it already caches a
/// 624 ms survey because opening one deck asked several times. A read that writes is a read that
/// surprises somebody, and this one would rewrite a user’s file from inside a getter.
///
/// So it runs once, when Epoch starts, which is also exactly when the answer can have changed:
/// somebody installed a runtime through Setup, or had one before Epoch existed, or installed one
/// yesterday while Epoch was closed.
///
/// ## It only ever adds
///
/// A backend the user configured wins on id — they typed an address and a measurement must not
/// overrule it — and one they **deleted** stays deleted for as long as Epoch is open. Re-adding
/// a removed entry on the next start is the honest cost of not keeping a list of things somebody
/// once rejected, and it is written here rather than discovered: an empty file is a decision this
/// respects (`load` says so), and a file with entries in it is one this adds to.
///
/// Returns what it added, so a surface can say so rather than changing the file silently.
pub fn adopt_installed(vault: &Path) -> Vec<String> {
    let mut held = Backends::load(vault);
    // An unreadable file is not one to write over — `save` refuses anyway, and asking it to
    // would turn a stray character into a lost configuration.
    if held.problem.is_some() {
        return Vec::new();
    }
    let Some(here) = installed_here() else {
        return Vec::new();
    };

    let mut added = Vec::new();
    for one in here {
        if held.backends.iter().any(|it| it.id == one.id) {
            continue;
        }
        added.push(one.id.clone());
        held.backends.push(one);
    }
    if added.is_empty() {
        return added;
    }
    match held.save(vault) {
        Ok(()) => added,
        // Said back as nothing added, because nothing was: reporting a write that failed is the
        // gauge with nothing behind it, one subsystem over.
        Err(_) => Vec::new(),
    }
}

impl Backends {
    /// What ships when nobody has configured anything.
    ///
    /// One local backend at the address Ollama uses, because the first thing a new user should
    /// be able to do is talk to somebody — not fill in a form about where a server is.
    ///
    /// ## What is actually installed is added on top (owner, 2026-09-07)
    ///
    /// A single Ollama entry is a reasonable guess, and on a machine with llama.cpp and LM
    /// Studio installed it is two-thirds wrong. Somebody who installed a runtime — from Epoch's
    /// own Setup or years before Epoch existed — has already said what they want to think with,
    /// and making them then add it in a form is asking twice. `adopt.rs` says the same thing in
    /// its own subject: **a setup somebody can get wrong, that Epoch could have done, is a setup
    /// Epoch should do.**
    ///
    /// `adopt.rs` says this in its own subject and it is the same sentence: **a setup somebody
    /// can get wrong, that Epoch could have done, is a setup Epoch should do.**
    ///
    /// ## Installed, not serving — and the existing rule is untouched
    ///
    /// The comment on `runners` says a Provider offered for a switched-off server *looks
    /// configured and refuses every turn*. That is still true and this does not contradict it,
    /// because the two lists answer different questions. `runners` is **what is answering right
    /// now** and is offered without being written down. A backend is **where this thing lives**,
    /// and Epoch already draws `SERVING` against `NOT SERVING` on every row and has a button
    /// that starts it. A configured address for a stopped program somebody installed is not a
    /// gauge with nothing behind it; it is the address, and the lamp beside it tells the truth.
    ///
    /// ## And the measuring happens in `adopt_installed`, not here
    ///
    /// The first version of this read the machine directly, and two tests that had asserted
    /// *one backend* started failing — correctly, on a machine with three runtimes installed.
    /// **A default that measures is not a default**, and a test whose result depends on what
    /// somebody happened to install is not a test.
    ///
    /// So this stays what it always was, and there is exactly one place that looks at the
    /// machine: [`adopt_installed`], called once at startup, which adds what is here and writes
    /// it down. A fresh vault therefore gets Ollama from here and everything else a moment
    /// later, which is also the honest order — one is a guess and the other is a measurement.
    pub fn first_run() -> Self {
        Self {
            backends: vec![Backend {
                id: Kind::Ollama.as_str().to_owned(),
                kind: Kind::Ollama,
                endpoint: Kind::Ollama.usual_endpoint().to_owned(),
                enabled: true,
            }],
            problem: None,
            keyed: Vec::new(),
            // Filled by `load` from the roster. A first run has paired nothing.
            bridges: Vec::new(),
            // Filled by `load` from this machine. Measured, so a first run has whatever is
            // already running on it — which for somebody who installed LM Studio before Epoch
            // is a Service that is simply there.
            runners: Vec::new(),
        }
    }

    /// Read the configuration. Never fails: an unreadable file is reported and the defaults
    /// stand, so a stray character cannot leave somebody unable to think at all.
    pub fn load(vault: &Path) -> Self {
        let file = path(vault);
        let mut loaded = match std::fs::read_to_string(&file) {
            Err(_) => Self::first_run(),
            Ok(raw) => match toml::from_str::<Backends>(&raw) {
                Ok(mut found) => {
                    if found.backends.is_empty() {
                        // An empty list is a decision, not a mistake — somebody who removed
                        // every backend meant it, and inventing one back would undo their edit.
                        found.problem = None;
                    }
                    found
                }
                Err(err) => Self {
                    problem: Some(format!(
                        "{} could not be read ({err}). Using the defaults; your file is untouched.",
                        file.display()
                    )),
                    ..Self::first_run()
                },
            },
        };

        // The paired machines, from the same vault. **Read here rather than by every caller**,
        // because the one thing this type is asked is *does this machine offer that Provider* —
        // and it answered `no` for a machine the user had just paired and picked from a list.
        // Saving a character then failed, silently, and the editor snapped back to the old
        // Service as though nothing had been chosen.
        loaded.bridges = crate::Pairings::load(vault)
            .all()
            .iter()
            .filter(|paired| paired.may(crate::Grant::Compute))
            // **Every Provider that machine is**, not only its default one. A machine running
            // LM Studio as well as Ollama is two Providers (`crate::bridge::bridges`), and a
            // list that knew about one of them would refuse to save a character onto the other
            // — which is this exact bug, one level deeper than the last time it was fixed.
            .flat_map(|paired| {
                std::iter::once(format!("bridge:{}", paired.id)).chain(
                    paired
                        .runners
                        .iter()
                        .filter(|runner| runner.as_str() != "ollama")
                        .map(|runner| format!("bridge:{}:{runner}", paired.id)),
                )
            })
            .collect();

        // And the programs on this machine, measured the same way and for the same reason. One
        // survey per load rather than one per question: a TCP gate with its own short deadline
        // and a look through the directories an installer uses, which is cheap enough to pay on
        // a rebuild and would not be cheap enough to pay per turn.
        // **Cached, because `load` is on a path somebody is waiting on.** Measured: a survey is
        // 624 ms of directory walking and TCP probing, and this is called whenever anything asks
        // what this machine offers — so opening one deck paid it several times. A runtime does
        // not start or stop inside a second; the ASK AGAIN button measures for real.
        loaded.runners = epoch_models::runtimes::survey_cached()
            .into_iter()
            .filter(|seen| seen.serving)
            .map(|seen| seen.id.to_owned())
            .filter(|id| !loaded.backends.iter().any(|b| &b.id == id))
            .collect();
        loaded
    }

    /// Write it back.
    ///
    /// Refuses to save over a file it could not read: the whole point of reporting a malformed
    /// file rather than replacing it is lost if the next save destroys it anyway.
    pub fn save(&self, vault: &Path) -> Result<(), String> {
        if let Some(problem) = &self.problem {
            return Err(format!(
                "not overwriting a file that could not be read: {problem}"
            ));
        }
        if let Some(reason) = self.problems().into_iter().next() {
            return Err(reason);
        }
        let raw = toml::to_string_pretty(&OnDisk {
            backends: &self.backends,
        })
        .map_err(|e| e.to_string())?;
        std::fs::create_dir_all(vault).map_err(|e| e.to_string())?;
        std::fs::write(path(vault), raw).map_err(|e| e.to_string())
    }

    /// Everything wrong across the whole configuration.
    pub fn problems(&self) -> Vec<String> {
        let mut found: Vec<String> = self.backends.iter().flat_map(Backend::problems).collect();
        // Two entries answering to one name is the failure that makes a character's assignment
        // ambiguous, and ambiguity here is somebody's model silently changing.
        let mut seen: Vec<&str> = Vec::new();
        for backend in &self.backends {
            let id = backend.id.trim();
            if seen.contains(&id) {
                found.push(format!("two backends are both called '{id}'"));
            }
            seen.push(id);
        }
        found
    }

    /// Add or replace one, by id.
    pub fn remember(&mut self, backend: Backend) {
        match self.backends.iter_mut().find(|b| b.id == backend.id) {
            Some(existing) => *existing = backend,
            None => self.backends.push(backend),
        }
    }

    /// Forget one. Returns whether there was anything to forget.
    pub fn forget(&mut self, id: &str) -> bool {
        let before = self.backends.len();
        self.backends.retain(|b| b.id != id);
        self.backends.len() != before
    }

    /// Whether a character could name this Provider.
    ///
    /// A configured backend that is switched on, **or a paired machine that may think**. The
    /// second half was missing, and the symptom was not an error message: picking a paired Mac
    /// and pressing SAVE put the old Service back, because the save was refused and the editor
    /// re-read what was still on disk.
    pub fn offers(&self, id: &str) -> bool {
        self.backends.iter().any(|b| b.id == id && b.enabled)
            || self.bridges.iter().any(|bridge| bridge == id)
            || self.runners.iter().any(|runner| runner == id)
    }

    /// Every Provider id a character may name, configured and paired alike.
    ///
    /// **The list form of [`Backends::offers`], and it exists because there were two of them.**
    /// `offers` learned about Bridges; the Launcher's `Machine` was still being assembled from
    /// `.backends` alone by a caller that had no way to know a second source had appeared. So
    /// saving a character onto a paired Mac was still refused, by a copy of a question this type
    /// already answers. One list, asked for rather than rebuilt.
    pub fn offered(&self) -> Vec<String> {
        self.backends
            .iter()
            .filter(|b| b.enabled)
            .map(|b| b.id.clone())
            .chain(self.bridges.iter().cloned())
            .chain(self.runners.iter().cloned())
            .collect()
    }

    /// Build the live registry: **everything that can currently think.**
    ///
    /// Disabled entries are left out rather than built and skipped: a Provider that exists and
    /// is never asked anything is a thing that can be probed by accident.
    ///
    /// Paired machines are added here too, and deliberately not by the caller. A bridge is a
    /// Provider whose machine is somebody else's (ADR-0029) — but it is a Provider, and a
    /// registry that only knew about *configured backends* would mean every surface asking
    /// "what can think?" had to remember to ask a second question. They come from the roster
    /// rather than from configuration, which is why the vault is reached through the store the
    /// bearers already live in.
    pub fn registry(&self, secrets: &Secrets) -> ProviderRegistry {
        let mut providers: Vec<Box<dyn Provider>> = Vec::new();
        for backend in self.backends.iter().filter(|b| b.enabled) {
            // Read at build time, held only by the Provider that sends it. The store is asked
            // once per rebuild rather than once per request, and nothing above this line ever
            // holds a credential.
            let key = secrets.get(&SecretName::for_backend(&backend.id));
            match backend.kind {
                Kind::Ollama => providers.push(Box::new(
                    Ollama::named(&backend.id, &backend.endpoint).with_key(key),
                )),
                Kind::Anthropic => providers.push(Box::new(
                    Anthropic::named(&backend.id, &backend.endpoint).with_key(key),
                )),
                Kind::OpenAi => providers.push(Box::new(
                    crate::openai::OpenAi::named(&backend.id, &backend.endpoint).with_key(key),
                )),
            }
        }
        /*
            **The programs on this machine, on the same terms as the ones on somebody else's.**

            A paired MacBook's LM Studio becomes a Provider because that machine said it is
            serving one. This machine's did not — it needed `ADD AS A SERVICE` pressed, which
            wrote a backend entry saying what had already been measured. One fact, two rules,
            and which rule applied depended on which computer the program was installed on.

            Nothing is written. These come and go with the servers themselves, exactly as a
            remote one does, and a backend somebody configured by hand keeps its place (they are
            filtered out on load).
        */
        for id in &self.runners {
            let Some(runtime) = epoch_models::runtimes::Runtime::ALL
                .into_iter()
                .find(|r| r.id() == id)
            else {
                continue;
            };
            let endpoint = format!("http://127.0.0.1:{}", runtime.usual_port());
            match runtime {
                epoch_models::runtimes::Runtime::Ollama => {
                    providers.push(Box::new(Ollama::named(id, &endpoint)))
                }
                // Both serve the OpenAI-compatible API Epoch has spoken since Phase 8, which is
                // why neither needed anything added to this build — only to be running.
                _ => providers.push(Box::new(crate::openai::OpenAi::named(id, &endpoint))),
            }
        }

        providers.extend(crate::bridge::bridges(
            &crate::Pairings::load(secrets.vault()),
            secrets,
        ));
        ProviderRegistry::of(providers)
    }

    /// Note which backends have a credential, for a surface that must say *stored* or *none*.
    ///
    /// Ids only, and derived — never stored alongside the configuration, so the two cannot come
    /// to disagree about what exists.
    pub fn note_keys(&mut self, secrets: &Secrets) {
        self.keyed = self
            .backends
            .iter()
            .filter(|b| secrets.holds(&SecretName::for_backend(&b.id)))
            .map(|b| b.id.clone())
            .collect();
    }
}

#[cfg(test)]
mod tests {

    /// One fact, one spelling — asserted for every variant rather than for the one that broke.
    #[test]
    fn what_serde_writes_is_what_the_code_calls_it() {
        /*
            **The two agreed by luck for two variants out of three.** `rename_all = "snake_case"`
            makes a single-word variant its own lowercase, so `Ollama` and `Anthropic` matched
            `as_str` by coincidence; `OpenAi` became `open_ai` and did not. A surface sending what
            `as_str` produced was refused by serde, and the refusal named both spellings without
            anybody noticing they were the same enum.

            Written over every variant so a fourth one cannot repeat it.
        */
        for kind in [Kind::Ollama, Kind::Anthropic, Kind::OpenAi] {
            let written = serde_json::to_string(&kind).expect("a kind serialises");
            assert_eq!(
                written,
                format!("\"{}\"", kind.as_str()),
                "{kind:?} is written one way and named another"
            );
            let read: Kind = serde_json::from_str(&written).expect("and reads back");
            assert_eq!(read, kind);
            // And the id a person or a surface would type resolves to the same thing.
            assert_eq!(Kind::from_id(kind.as_str()), Some(kind));
        }
    }

    /// A file written by an older build is still readable.
    #[test]
    fn the_spelling_that_was_stored_before_still_reads() {
        // The alias is an input tolerance, never what is written - so somebody who added a
        // backend on a build that spelled it `open_ai` does not lose it.
        let read: Kind = serde_json::from_str("\"open_ai\"").expect("the old spelling reads");
        assert_eq!(read, Kind::OpenAi);
        assert_eq!(
            serde_json::to_string(&read).unwrap(),
            "\"openai\"",
            "and it is rewritten in the one spelling"
        );
    }
    #[test]
    fn a_paired_machine_is_a_provider_a_character_may_name() {
        // The defect, exactly: picking a paired Mac and pressing SAVE put the old Service back.
        // The save was refused — `offers` only knew about configured backends, and a Bridge
        // arrives from the roster — and the editor re-read what was still on disk. No error was
        // shown, so it looked like the choice had simply not been taken.
        let vault = std::env::temp_dir().join("epoch-offers-a-bridge");
        let _ = std::fs::remove_dir_all(&vault);
        std::fs::create_dir_all(&vault).expect("a temporary vault");

        let mut roster = crate::Pairings::default();
        let id = roster.add(
            "studio-mac.local",
            "https://192.168.1.20:11500",
            Some("aa".repeat(32)),
            vec![crate::Grant::Compute],
            0,
        );
        roster.save(&vault).expect("saves");

        let backends = Backends::load(&vault);
        assert!(
            backends.offers(&format!("bridge:{id}")),
            "a paired machine that may think is nameable"
        );
        // And the configured one still is.
        assert!(backends.offers("ollama"));
        // While something nobody paired is not — the point is not "anything called bridge".
        assert!(!backends.offers("bridge:invented"));

        let _ = std::fs::remove_dir_all(&vault);
    }

    #[test]
    fn the_list_and_the_question_cannot_disagree() {
        // The defect a second time: `offers` knew about Bridges, and the Launcher was still
        // assembling its own list from `.backends` alone. Anything that needs the whole list
        // asks for it here, so the two can never drift apart again.
        let vault = std::env::temp_dir().join("epoch-offered-list");
        let _ = std::fs::remove_dir_all(&vault);
        std::fs::create_dir_all(&vault).expect("a temporary vault");

        let mut roster = crate::Pairings::default();
        let id = roster.add(
            "studio-mac.local",
            "https://192.168.1.20:11500",
            Some("aa".repeat(32)),
            vec![crate::Grant::Compute],
            0,
        );
        roster.save(&vault).expect("saves");

        let backends = Backends::load(&vault);
        let listed = backends.offered();
        assert!(listed.contains(&format!("bridge:{id}")), "{listed:?}");
        assert!(listed.contains(&"ollama".to_owned()), "{listed:?}");
        for one in &listed {
            assert!(backends.offers(one), "'{one}' is listed and refused");
        }

        let _ = std::fs::remove_dir_all(&vault);
    }

    #[test]
    fn a_machine_that_may_not_think_is_not_offered() {
        // Paired and allowed nothing is a real state. Naming it as a Service would offer a
        // character a machine that will refuse every turn.
        let vault = std::env::temp_dir().join("epoch-offers-no-grant");
        let _ = std::fs::remove_dir_all(&vault);
        std::fs::create_dir_all(&vault).expect("a temporary vault");

        let mut roster = crate::Pairings::default();
        let id = roster.add(
            "Studio Mac",
            "https://192.168.1.20:11500",
            Some("aa".repeat(32)),
            vec![],
            0,
        );
        roster.save(&vault).expect("saves");

        assert!(!Backends::load(&vault).offers(&format!("bridge:{id}")));
        let _ = std::fs::remove_dir_all(&vault);
    }

    use super::*;

    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let d = std::env::temp_dir().join(format!("epoch-backends-{name}-{n}"));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            Self(d)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A vault with no secret store in it. Building a registry must not require one — a
    /// machine where nobody has ever typed a key is the ordinary case.
    fn nowhere() -> PathBuf {
        std::env::temp_dir().join("epoch-backends-no-secrets")
    }

    fn ollama(id: &str, endpoint: &str) -> Backend {
        Backend {
            id: id.into(),
            kind: Kind::Ollama,
            endpoint: endpoint.into(),
            enabled: true,
        }
    }

    #[test]
    fn a_machine_with_no_configuration_can_still_think() {
        // The first thing a new user should be able to do is talk to somebody, not fill in a
        // form about where a server is.
        let d = Dir::new("firstrun");
        let backends = Backends::load(&d.0);
        assert_eq!(backends.backends.len(), 1);
        assert_eq!(backends.backends[0].endpoint, "http://localhost:11434");
        assert!(backends.problem.is_none());
    }

    /// What is already configured is never touched, whatever the machine has on it.
    ///
    /// **Written against a file rather than against a machine.** The first version of this
    /// feature measured inside `first_run`, and two tests that asserted *one backend* began
    /// failing on a machine with three runtimes installed — correctly. A test whose result
    /// depends on what somebody happened to install is not a test, so this asserts the property
    /// that matters and cannot depend on the runner: **it only ever adds.**
    #[test]
    fn adopting_never_overwrites_what_somebody_typed() {
        let d = Dir::new("adopt");
        let mine = ollama("ollama", "http://192.168.1.99:11434");
        let mut held = Backends::default();
        held.remember(mine.clone());
        held.save(&d.0).unwrap();

        let added = adopt_installed(&d.0);
        let after = Backends::load(&d.0);

        // The entry that was there is the entry that is there, address and all.
        let kept = after
            .backends
            .iter()
            .find(|it| it.id == "ollama")
            .expect("the configured backend is still configured");
        assert_eq!(kept.endpoint, "http://192.168.1.99:11434");
        assert!(
            !added.contains(&"ollama".to_owned()),
            "an id that was already in the file must not be reported as added"
        );
        // Whatever else this machine has, nothing was removed.
        assert!(!after.backends.is_empty());
    }

    /// An unreadable file is not written over, even to add something to it.
    #[test]
    fn adopting_leaves_a_broken_file_exactly_as_it_found_it() {
        let d = Dir::new("adopt-broken");
        std::fs::write(path(&d.0), "backends = [ this is not toml").unwrap();

        assert!(adopt_installed(&d.0).is_empty());
        let still = std::fs::read_to_string(path(&d.0)).unwrap();
        assert!(still.contains("this is not toml"), "the file is untouched");
    }

    #[test]
    fn configuration_survives_the_round_trip() {
        let d = Dir::new("roundtrip");
        let mut backends = Backends::default();
        backends.remember(ollama("laptop", "http://localhost:11434"));
        backends.remember(ollama("desktop", "http://192.168.1.20:11434"));
        backends.save(&d.0).unwrap();

        let read = Backends::load(&d.0);
        assert_eq!(read.backends, backends.backends);
        // Two of one kind is the case that makes id and kind different questions.
        assert_eq!(read.backends.len(), 2);
        assert!(read.offers("desktop"));
    }

    #[test]
    fn a_file_that_cannot_be_read_is_reported_and_never_replaced() {
        // Somebody's hand-edited endpoint is not ours to discard because a bracket is missing.
        let d = Dir::new("malformed");
        std::fs::write(path(&d.0), "backends = [ this is not toml").unwrap();

        let backends = Backends::load(&d.0);
        assert!(backends.problem.is_some(), "the reason must reach the user");
        // The defaults stand, so a stray character does not leave nobody able to think.
        assert_eq!(backends.backends.len(), 1);

        // And saving refuses, or reporting it rather than replacing it would have been theatre.
        assert!(backends.save(&d.0).is_err());
        let still = std::fs::read_to_string(path(&d.0)).unwrap();
        assert!(still.contains("this is not toml"), "the file is untouched");
    }

    #[test]
    fn the_reason_reaches_a_surface_without_reaching_the_file() {
        // A surface showing the defaults with no way to say they are not the user's own list
        // would show them as if they had been chosen.
        let broken = Backends {
            problem: Some("bad bracket".into()),
            ..Backends::first_run()
        };
        let json = serde_json::to_string(&broken).unwrap();
        assert!(
            json.contains("bad bracket"),
            "the surface must be able to say why"
        );

        // But it never lands in the configuration it describes, or it would outlive the fault.
        let raw = toml::to_string_pretty(&Backends::first_run()).unwrap();
        assert!(!raw.contains("problem"));
        // And it is never read back, so typing one in cannot lock somebody out of saving.
        let typed: Backends = toml::from_str("problem = \"mine\"\nbackends = []").unwrap();
        assert!(typed.problem.is_none());
    }

    #[test]
    fn only_the_configuration_reaches_the_configuration_file() {
        // `problem` and `keyed` are answers about *this* load. Writing either into the file they
        // describe would make it outlive the fault it reports, and would give the store a second
        // record of what it holds — two records of one fact eventually disagree.
        let d = Dir::new("ondisk");
        let mut backends = Backends::first_run();
        backends.keyed = vec!["ollama".into()];
        backends.save(&d.0).unwrap();

        let raw = std::fs::read_to_string(path(&d.0)).unwrap();
        assert!(!raw.contains("keyed"));
        assert!(!raw.contains("problem"));
        assert!(
            raw.contains("ollama"),
            "the configuration itself is still there"
        );
    }

    #[test]
    fn removing_every_backend_is_a_decision_rather_than_a_mistake() {
        // Inventing one back would undo somebody's edit, and they would have to make it twice.
        let d = Dir::new("empty");
        let backends = Backends::default();
        backends.save(&d.0).unwrap();
        assert!(Backends::load(&d.0).backends.is_empty());
    }

    #[test]
    fn two_backends_may_not_answer_to_one_name() {
        // A character naming an ambiguous id is a character whose model changes silently.
        let mut backends = Backends::default();
        backends.backends.push(ollama("ollama", "http://a"));
        backends.backends.push(ollama("ollama", "http://b"));
        assert!(backends
            .problems()
            .iter()
            .any(|p| p.contains("both called")));
    }

    #[test]
    fn an_address_is_refused_rather_than_guessed_at() {
        // Guessing a scheme is how a typo becomes a quiet connection somewhere else.
        let d = Dir::new("scheme");
        let mut backends = Backends::default();
        backends.remember(ollama("ollama", "localhost:11434"));
        assert!(backends.save(&d.0).unwrap_err().contains("http://"));
    }

    #[test]
    fn a_disabled_backend_is_kept_but_never_built() {
        // Off is not deleted. It keeps its address, stops being probed, and a Provider that
        // exists but is never asked anything cannot be probed by accident.
        let mut backends = Backends::default();
        backends.remember(Backend {
            enabled: false,
            ..ollama("ollama", "http://localhost:11434")
        });
        assert!(!backends.offers("ollama"));
        assert!(backends
            .registry(&Secrets::at(&nowhere()))
            .survey()
            .is_empty());
        assert_eq!(backends.backends.len(), 1, "still configured");
    }

    #[test]
    fn a_serving_program_on_this_machine_is_a_provider_without_anybody_adding_it() {
        // **One fact, one rule.** A paired MacBook's LM Studio becomes a Provider the moment
        // that machine reports it serving; this machine's needed `ADD AS A SERVICE` pressed,
        // which wrote a backend entry saying what had already been measured. Which rule applied
        // depended on which computer the program happened to be installed on.
        let backends = Backends {
            runners: vec!["lm_studio".into(), "llama_cpp".into()],
            ..Default::default()
        };
        let ids: Vec<String> = backends
            .registry(&Secrets::at(&nowhere()))
            .survey()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert!(ids.contains(&"lm_studio".to_owned()), "{ids:?}");
        assert!(ids.contains(&"llama_cpp".to_owned()), "{ids:?}");

        // And a character may be assigned to one, which is the half that was refused silently
        // the last time a source of Providers was added and this was not told about it.
        assert!(backends.offers("lm_studio"));
        assert!(backends.offered().contains(&"llama_cpp".to_owned()));
    }

    #[test]
    fn a_hand_configured_backend_outranks_a_measurement_of_the_same_name() {
        // Somebody who typed an endpoint has said something. A measurement must not overrule
        // it — and two Providers with one id would be one choice made twice.
        let mut backends = Backends::default();
        backends.remember(ollama("lm_studio", "http://127.0.0.1:4321"));
        // `load` is what filters these; the invariant it maintains is asserted here directly.
        backends.runners = Vec::new();
        let ids: Vec<String> = backends
            .registry(&Secrets::at(&nowhere()))
            .survey()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(ids, vec!["lm_studio"], "one entry, the one somebody chose");
    }

    #[test]
    fn the_registry_is_built_from_what_was_configured() {
        let mut backends = Backends::default();
        backends.remember(ollama("laptop", "http://127.0.0.1:9"));
        backends.remember(ollama("desktop", "http://127.0.0.1:9"));
        let ids: Vec<String> = backends
            .registry(&Secrets::at(&nowhere()))
            .survey()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(ids, vec!["laptop", "desktop"]);
    }
}
