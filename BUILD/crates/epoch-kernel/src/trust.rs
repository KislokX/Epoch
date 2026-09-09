//! The Trust Engine — what a Quest is allowed to actually do (ADR-0009).
//!
//! ## The decision is a pure function
//!
//! Given what the capability is, what mode the World is in, and what the user has already
//! decided, the verdict follows. No clock, no filesystem, no network, no state. That makes the
//! single most safety-critical judgement in Epoch exhaustively testable headless — and it means
//! a surface can never reach a different conclusion than the Engine, because there is only one
//! conclusion to reach.
//!
//! ## Three gates, and they are independent
//!
//! ```text
//! Mode      what this World is currently for      Manual · Accept edits · Auto
//! Policy    what the user has already decided     Allow · Deny, at a scope
//! Place     where the work is allowed to happen   the Project Root (checked elsewhere)
//! ```
//!
//! Being in Auto mode does not put you inside the Project Root; being inside the Project
//! Root does not grant you the shell. Collapsing them into one permission is how a system ends
//! up with a single "dangerous mode" switch, which is the opposite of understandable autonomy.
//!
//! The third gate is confinement and lives with whatever knows about paths — a capability
//! cannot be confined by a rule that has never seen its arguments.
//!
//! ## It fails closed, everywhere
//!
//! A capability nobody registered is refused. A `Deny` beats everything, including Auto.
//! An unknown situation asks rather than proceeds. The default for anything not thought about
//! is **stop**, because the failure mode on the other side is a model deleting somebody's work.
//!
//! ## Trust is granted by people, never by content
//!
//! Nothing here can be changed by a model, a web page, a file or a tool result. A [`Policy`]
//! exists only because a person clicked something. This is stated in ADR-0009 as a mitigation
//! and it is enforced by the fact that the only way in is a user command.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::capability::{Descriptor, Effect, Explanation, Risk};
use crate::definition::CharacterId;

/// How much this World may do on its own right now.
///
/// One legible dial rather than a permission matrix, named for **what happens** rather than for
/// what the user is (ADR-0009 amendment, 2026-07-31).
///
/// Ordered from least to most permitted, and the order is used: it makes "stricter than" a
/// comparison rather than a lookup table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
// snake_case, not lowercase: `AcceptEdits` must round-trip as `accept_edits`, which is what
// `as_str` and `from_id` use. Two spellings of one mode is a stored setting that stops loading.
#[serde(rename_all = "snake_case")]
pub enum Autonomy {
    /// Say what it is about to do, and wait. Reading still never interrupts.
    ///
    /// A `Plan` mode existed here briefly and was removed: it refused rather than asked, and
    /// from the user's seat that is the same experience with one more word to learn. Manual
    /// already plans — it states the action and stops — so a second mode for "plan first" was
    /// a distinction only the implementation could feel.
    ///
    /// What is given up, honestly: there is no longer a mode that *cannot* be clicked through.
    /// Somebody who wants "nothing here can change, ever" now relies on not misclicking. Three
    /// modes people understand beat four that need explaining (`CLAUDE.md` — great defaults
    /// beat endless configuration).
    #[serde(alias = "plan", alias = "explorer", alias = "builder")]
    Manual,
    /// File changes proceed. Deleting, running and reaching the network still ask.
    ///
    /// Not a risk level: a capability qualifies by **what it touches**, not by how much it
    /// could cost. "Accept edits" is a promise about files, and a capability that also opens a
    /// socket is not a file edit however cheap it looks.
    AcceptEdits,
    /// Free use of what this character was given. A standing `Deny` still wins.
    ///
    /// The freedom is over the tools they already have — never over which tools they have.
    #[serde(alias = "autonomous")]
    Auto,
}

impl Autonomy {
    pub const ALL: [Autonomy; 3] = [Autonomy::Manual, Autonomy::AcceptEdits, Autonomy::Auto];

    pub fn as_str(self) -> &'static str {
        match self {
            Autonomy::Manual => "manual",
            Autonomy::AcceptEdits => "accept_edits",
            Autonomy::Auto => "auto",
        }
    }

    /// What a surface calls it.
    pub fn label(self) -> &'static str {
        match self {
            Autonomy::Manual => "Manual",
            Autonomy::AcceptEdits => "Accept edits",
            Autonomy::Auto => "Auto",
        }
    }

    /// Accept a choice arriving from a surface, or refuse it.
    ///
    /// Retired names still resolve, so a `trust.toml` written before this rename keeps working.
    /// A rename that bricks a stored permission setting fails in the worst possible direction:
    /// the file stops parsing, every standing decision is dropped, and it looks like nothing is
    /// wrong.
    pub fn from_id(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "manual" | "plan" | "explorer" | "builder" => Some(Autonomy::Manual),
            "accept_edits" => Some(Autonomy::AcceptEdits),
            "auto" | "autonomous" => Some(Autonomy::Auto),
            _ => None,
        }
    }

    /// One line the user can read before choosing.
    ///
    /// **Reading is free in every mode**, and every line here has to say so or imply it. The
    /// first version of Manual read "says what it will do, then waits for you", which promised
    /// more than the gate delivers: `read_file` is an observation, and observations are allowed
    /// before the mode is even consulted. Somebody who picks the strictest mode and then watches
    /// a file get read has been told something untrue about their own machine — and the fix is
    /// the sentence, not the rule. Asking on every read would make Manual unusable and train
    /// people to approve without looking, which is how a real approval gets clicked through.
    pub fn describe(self) -> &'static str {
        match self {
            Autonomy::Manual => "Asks before anything changes. Reading the project is free.",
            Autonomy::AcceptEdits => {
                "Writes and edits go ahead. Running, deleting and the network still ask."
            }
            Autonomy::Auto => "Free use of the tools this character has.",
        }
    }

    /// The highest risk this mode lets through **without asking**.
    ///
    /// `None` means "nothing above observation", not "nothing at all" — reading is the floor of
    /// usefulness and never interrupts anybody.
    fn ceiling(self) -> Risk {
        match self {
            Autonomy::Manual | Autonomy::AcceptEdits => Risk::None,
            Autonomy::Auto => Risk::High,
        }
    }

    /// Whether this mode waves a capability through on the strength of *what it touches*.
    ///
    /// Only `AcceptEdits`, and only for things that read and write files. The moment a
    /// capability also deletes, executes or opens a socket it stops being a file edit — and
    /// somebody who ticked "accept edits" was not agreeing to any of those.
    fn waves_through(self, descriptor: &Descriptor) -> bool {
        self == Autonomy::AcceptEdits
            && descriptor
                .effects
                .iter()
                .all(|e| matches!(e, Effect::Reads | Effect::Writes))
    }
}

impl Default for Autonomy {
    /// Manual. Anything looser is a decision about somebody's files that they did not make;
    /// anything tighter makes Epoch useless out of the box and trains people to change the
    /// setting before they understand it.
    fn default() -> Self {
        Autonomy::Manual
    }
}

impl std::fmt::Display for Autonomy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How widely a decision applies — the "Always allow for…" of ADR-0009.
///
/// Ordered narrowest first, and the order is load-bearing: a decision about one person beats a
/// decision about the whole World, so "Robo may run commands" does not become "everybody may".
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "scope", content = "who")]
pub enum Scope {
    /// This one crew member, wherever they work.
    Character(CharacterId),
    /// Everyone working in this World.
    World(String),
    /// Everywhere. Deliberately last and deliberately hard to reach from a surface.
    Everywhere,
}

impl Scope {
    /// Whether this scope covers a particular piece of work.
    fn covers(&self, character: &CharacterId, world: &str) -> bool {
        match self {
            Scope::Character(id) => id == character,
            Scope::World(id) => id == world,
            Scope::Everywhere => true,
        }
    }

    /// Narrow beats wide. A `Deny` for one person is not overridden by an `Allow` for the World.
    fn precedence(&self) -> u8 {
        match self {
            Scope::Character(_) => 0,
            Scope::World(_) => 1,
            Scope::Everywhere => 2,
        }
    }
}

/// What the user decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Allow,
    /// Never, in this scope. Beats every mode, including Autonomous.
    Deny,
}

/// A standing decision by the user about one capability.
///
/// **Only a person creates one of these.** Not a model, not a tool result, not a file. That is
/// the mitigation ADR-0009 names against prompt injection escalating trust, and it holds
/// because the only path in is a user command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    pub capability: String,
    pub scope: Scope,
    pub decision: Decision,
}

impl Policy {
    pub fn allow(capability: &str, scope: Scope) -> Self {
        Self {
            capability: capability.to_owned(),
            scope,
            decision: Decision::Allow,
        }
    }

    pub fn deny(capability: &str, scope: Scope) -> Self {
        Self {
            capability: capability.to_owned(),
            scope,
            decision: Decision::Deny,
        }
    }
}

/// Who wants to do what, where. Everything the decision depends on besides the capability.
#[derive(Debug, Clone, PartialEq)]
pub struct Situation<'a> {
    pub character: &'a CharacterId,
    /// World id. What "always allow for this project" is keyed on.
    pub world: &'a str,
    pub mode: Autonomy,
}

/// Why something was allowed. Kept so History can say more than "it happened".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Because {
    /// It changes nothing. The only thing that never needs asking.
    ItOnlyLooks,
    /// The mode covers this level of risk.
    TheModeAllowsIt(Autonomy),
    /// The user said so, earlier, at this scope.
    YouAllowedIt(Scope),
}

impl Because {
    pub fn describe(&self) -> String {
        match self {
            Because::ItOnlyLooks => "it changes nothing".into(),
            Because::TheModeAllowsIt(mode) => format!("{mode} mode allows it"),
            Because::YouAllowedIt(Scope::Character(who)) => format!("you allowed this for {who}"),
            Because::YouAllowedIt(Scope::World(id)) => format!("you allowed this in {id}"),
            Because::YouAllowedIt(Scope::Everywhere) => "you allowed this everywhere".into(),
        }
    }
}

/// What the Trust Engine says.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// Go ahead. The reason is carried so nothing downstream has to guess it.
    Allowed(Because),
    /// Tell the user what this would do and wait. Carries the sentence to show them.
    Ask(Box<Explanation>),
    /// No, and this is not a prompt. The reason is for the model as much as the user — it must
    /// learn it cannot do this rather than retrying forever.
    Refused(String),
}

impl Verdict {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Verdict::Allowed(_))
    }

    pub fn is_refused(&self) -> bool {
        matches!(self, Verdict::Refused(_))
    }
}

/// Decide whether one proposed execution may proceed.
///
/// Pure. Same inputs, same verdict, forever — which is what makes a safety gate reviewable.
///
/// Order matters and is fail-closed at every step:
///
/// 1. **Deny wins.** Before the mode, before anything. A "never" is never negotiated.
/// 2. **Observation never asks.** Reading is the floor of usefulness.
/// 3. **An explicit Allow is honoured** — this is what "always allow" means.
/// 4. **The mode lets routine work through** — by risk, or by what the capability touches.
/// 5. **Otherwise, ask.** Every situation nobody enumerated ends here, not in `Allowed`.
pub fn decide(
    descriptor: &Descriptor,
    explanation: &Explanation,
    situation: &Situation<'_>,
    policies: &[Policy],
) -> Verdict {
    let risk = descriptor.risk();

    // The strongest thing the user ever said about this capability, narrowest scope first.
    let standing = policies
        .iter()
        .filter(|p| p.capability == descriptor.id.as_str())
        .filter(|p| p.scope.covers(situation.character, situation.world))
        .min_by_key(|p| p.scope.precedence());

    // 1. Deny wins outright, whatever the mode. Including Autonomous.
    if let Some(policy) = standing {
        if policy.decision == Decision::Deny {
            return Verdict::Refused(format!(
                "you have told Epoch never to let {} do this",
                match &policy.scope {
                    Scope::Character(who) => who.to_string(),
                    Scope::World(id) => format!("anyone in {id}"),
                    Scope::Everywhere => "anyone".to_owned(),
                }
            ));
        }
    }

    // 2. Looking costs nothing and never interrupts anybody.
    if risk == Risk::None {
        return Verdict::Allowed(Because::ItOnlyLooks);
    }

    // 3. The user already said yes to this, at some scope.
    if let Some(policy) = standing {
        if policy.decision == Decision::Allow {
            return Verdict::Allowed(Because::YouAllowedIt(policy.scope.clone()));
        }
    }

    // 4. Routine work for this mode: by risk, or by what it touches.
    if risk <= situation.mode.ceiling() || situation.mode.waves_through(descriptor) {
        return Verdict::Allowed(Because::TheModeAllowsIt(situation.mode));
    }

    // 5. Anything else is the user's call, with the sentence to make it on.
    Verdict::Ask(Box::new(explanation.clone()))
}

/// Every capability that could possibly run in this situation.
///
/// **An allowlist, computed from the mode.** A capability added tomorrow and forgotten about
/// is excluded here by construction, because inclusion requires passing the gate rather than
/// avoiding a blocklist. A blocklist makes every oversight a hole; an allowlist makes every
/// oversight an inconvenience.
///
/// Used to decide what a model is even *told* it has. Something it is never offered cannot be
/// argued into.
pub fn offerable<'a>(
    descriptors: impl IntoIterator<Item = &'a Descriptor>,
    situation: &Situation<'_>,
    policies: &[Policy],
) -> BTreeSet<String> {
    descriptors
        .into_iter()
        .filter(|descriptor| {
            // An explanation nobody will show: the shape is what is being tested, not the text.
            let probe = Explanation::of(descriptor, descriptor.summary.clone());
            !decide(descriptor, &probe, situation, policies).is_refused()
        })
        .map(|d| d.id.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::capability::{CapabilityId, Effect, Reversal};

    fn who() -> CharacterId {
        CharacterId::new("robo").unwrap()
    }

    fn situation(mode: Autonomy) -> Situation<'static> {
        // Leaked deliberately: a test fixture that lives for the whole run keeps the borrow
        // simple and costs one allocation.
        static WHO: std::sync::OnceLock<CharacterId> = std::sync::OnceLock::new();
        Situation {
            character: WHO.get_or_init(who),
            world: "archipelago",
            mode,
        }
    }

    fn id(raw: &str) -> CapabilityId {
        CapabilityId::new(raw).unwrap()
    }

    fn reading() -> Descriptor {
        Descriptor::observing(id("read_file"), "Read a file.")
    }

    fn writing() -> Descriptor {
        Descriptor::acting(
            id("write_file"),
            "Write a file.",
            [Effect::Writes],
            Reversal::Undoable("the previous contents are kept".into()),
        )
    }

    fn deleting() -> Descriptor {
        Descriptor::acting(
            id("delete_file"),
            "Delete a file.",
            [Effect::Deletes],
            Reversal::Permanent,
        )
    }

    fn fetching() -> Descriptor {
        Descriptor::acting(
            id("fetch_url"),
            "Fetch a page.",
            [Effect::Reads, Effect::Network],
            Reversal::Permanent,
        )
    }

    fn verdict(descriptor: &Descriptor, mode: Autonomy, policies: &[Policy]) -> Verdict {
        let explanation = Explanation::of(descriptor, "doing the thing");
        decide(descriptor, &explanation, &situation(mode), policies)
    }

    #[test]
    fn looking_never_interrupts_anybody() {
        for mode in Autonomy::ALL {
            assert_eq!(
                verdict(&reading(), mode, &[]),
                Verdict::Allowed(Because::ItOnlyLooks)
            );
        }
    }

    #[test]
    fn a_retired_mode_name_still_loads_as_something_sensible() {
        // A rename that bricks a stored permission setting fails in the worst direction: the
        // file stops parsing, every standing decision is dropped, and nothing looks wrong.
        for retired in ["plan", "explorer", "builder"] {
            assert_eq!(
                Autonomy::from_id(retired),
                Some(Autonomy::Manual),
                "{retired}"
            );
        }
        assert_eq!(Autonomy::from_id("autonomous"), Some(Autonomy::Auto));
    }

    #[test]
    fn manual_asks_before_everything_that_is_not_looking() {
        for descriptor in [writing(), deleting(), fetching()] {
            assert!(
                matches!(verdict(&descriptor, Autonomy::Manual, &[]), Verdict::Ask(_)),
                "{}",
                descriptor.id
            );
        }
    }

    #[test]
    fn accept_edits_is_about_what_is_touched_not_what_it_costs() {
        // Writing is waved through even though it is Medium risk — the promise was about files.
        assert_eq!(
            verdict(&writing(), Autonomy::AcceptEdits, &[]),
            Verdict::Allowed(Because::TheModeAllowsIt(Autonomy::AcceptEdits))
        );
        // And the moment something also deletes or opens a socket it stops being a file edit,
        // whatever it costs. Somebody who ticked "accept edits" agreed to neither.
        assert!(matches!(
            verdict(&deleting(), Autonomy::AcceptEdits, &[]),
            Verdict::Ask(_)
        ));
        assert!(matches!(
            verdict(&fetching(), Autonomy::AcceptEdits, &[]),
            Verdict::Ask(_)
        ));
    }

    #[test]
    fn auto_uses_everything_the_character_was_given() {
        for descriptor in [writing(), deleting(), fetching()] {
            assert_eq!(
                verdict(&descriptor, Autonomy::Auto, &[]),
                Verdict::Allowed(Because::TheModeAllowsIt(Autonomy::Auto)),
                "{}",
                descriptor.id
            );
        }
    }

    #[test]
    fn auto_is_freedom_over_the_tools_they_have_and_never_over_which_tools() {
        // The gate Auto does not open: a standing Deny still wins, and a capability the
        // character never requested is never in the offer to begin with.
        let policies = [Policy::deny("delete_file", Scope::Everywhere)];
        assert!(verdict(&deleting(), Autonomy::Auto, &policies).is_refused());
    }

    #[test]
    fn a_standing_allow_is_what_always_allow_means() {
        let policies = [Policy::allow(
            "delete_file",
            Scope::World("archipelago".into()),
        )];
        assert_eq!(
            verdict(&deleting(), Autonomy::Manual, &policies),
            Verdict::Allowed(Because::YouAllowedIt(Scope::World("archipelago".into())))
        );
        // And it does not leak into another World.
        let elsewhere = Situation {
            world: "default",
            ..situation(Autonomy::Manual)
        };
        let explanation = Explanation::of(&deleting(), "doing the thing");
        assert!(matches!(
            decide(&deleting(), &explanation, &elsewhere, &policies),
            Verdict::Ask(_)
        ));
    }

    #[test]
    fn a_deny_beats_every_mode_including_auto() {
        let policies = [Policy::deny("delete_file", Scope::Everywhere)];
        for mode in Autonomy::ALL {
            assert!(verdict(&deleting(), mode, &policies).is_refused(), "{mode}");
        }
    }

    #[test]
    fn a_deny_about_one_person_beats_an_allow_about_the_whole_world() {
        // Narrow beats wide, so "Robo specifically may not" survives "anyone here may".
        let policies = [
            Policy::allow("delete_file", Scope::World("archipelago".into())),
            Policy::deny("delete_file", Scope::Character(who())),
        ];
        let v = verdict(&deleting(), Autonomy::Auto, &policies);
        assert!(v.is_refused(), "{v:?}");
    }

    #[test]
    fn a_policy_about_a_different_capability_does_nothing() {
        let policies = [Policy::allow("write_file", Scope::Everywhere)];
        assert!(matches!(
            verdict(&deleting(), Autonomy::Manual, &policies),
            Verdict::Ask(_)
        ));
    }

    #[test]
    fn the_offer_is_an_allowlist_so_a_forgotten_capability_is_excluded_not_included() {
        let all = [reading(), writing(), deleting(), fetching()];

        // Every mode is offered everything it could conceivably do, asking where it must. A
        // capability added tomorrow and never considered would still have to pass the gate to
        // appear here — the list is built by inclusion, not by remembering to exclude.
        for mode in Autonomy::ALL {
            assert_eq!(offerable(&all, &situation(mode), &[]).len(), 4, "{mode}");
        }

        // A denied capability is never even mentioned to the model. Something it is not
        // offered cannot be argued into.
        let denied = [Policy::deny("delete_file", Scope::Everywhere)];
        let guarded = offerable(&all, &situation(Autonomy::Manual), &denied);
        assert!(!guarded.contains("delete_file"));
        assert_eq!(guarded.len(), 3);
    }

    #[test]
    fn asking_carries_the_sentence_the_user_will_read() {
        let explanation = Explanation::of(&deleting(), "delete `notes/old.md`")
            .because("the plan says to remove the superseded note");
        match decide(&deleting(), &explanation, &situation(Autonomy::Manual), &[]) {
            Verdict::Ask(shown) => {
                assert_eq!(shown.what, "delete `notes/old.md`");
                assert_eq!(shown.risk, Risk::High);
                assert_eq!(shown.line(), "delete `notes/old.md` — cannot be undone");
                assert!(shown.why.is_some());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_mode_arriving_from_a_surface_is_refused_if_invented() {
        assert_eq!(Autonomy::from_id("manual"), Some(Autonomy::Manual));
        assert_eq!(
            Autonomy::from_id("  ACCEPT_EDITS "),
            Some(Autonomy::AcceptEdits)
        );
        assert_eq!(Autonomy::from_id("godmode"), None);
        // Every id round-trips, so a stored setting always loads back as itself.
        for mode in Autonomy::ALL {
            assert_eq!(Autonomy::from_id(mode.as_str()), Some(mode));
        }
    }
}
