//! Making somebody who did not exist.
//!
//! ## Why the crew starts at zero
//!
//! A first draft of the install design offered *"start with a crew of four"* — four archetype
//! templates copied in on request. That contradicts ADR-0023: **characters belong to the user.**
//! Handing somebody four pre-made people and calling them theirs is the same act as generating a
//! face for somebody who has not chosen one, one step earlier.
//!
//! So an empty vault stays empty, and the Launcher says so. `0 CREW` is the honest reading of the
//! instrument, and a World with nobody home is not a failure state — it is where authoring
//! starts (Build From Life, rule 1).
//!
//! ## What may be defaulted, and what may not
//!
//! One test: **does this decide who somebody is?**
//!
//! - **Defaulted freely** — the id (derived from the name), the initial routine (a character is
//!   never frozen, so the file would not load without one), no Worlds, no capabilities decided.
//!   None of it says anything about the person.
//! - **Never defaulted** — the brain. Picking a model would surprise somebody with whatever it
//!   costs; on this machine two local models differed by minutes on a cold start. An unassigned
//!   character says so, and that is already how [`epoch_kernel::CharacterDefinition::mind`] is
//!   documented.
//! - **Never invented** — the portrait and the sprite. `None` draws a visible stand-in rather
//!   than a generated likeness (ADR-0024).
//! - **Offered, not applied** — the prompt. An archetype can *suggest* words for somebody writing
//!   a character; the difference between help and imposition is who performed the act, so the
//!   suggestion is returned to the surface and only written if the user keeps it.

use epoch_kernel::{
    CharacterArchetype, CharacterDefinition, CharacterId, IdleBehavior, PresenceProfile,
};

use crate::definition::{DefinitionError, DefinitionRegistry};

/// A prompt an archetype can offer somebody who is writing a character.
///
/// Deliberately about **how they work**, never about who they are: no name, no backstory, no
/// personality the user did not ask for. It is a starting point they chose to see.
pub fn suggested_prompt(archetype: CharacterArchetype) -> &'static str {
    match archetype {
        CharacterArchetype::Researcher => {
            "You explore the solution space before committing, and you name tradeoffs explicitly.\n\
             You would rather understand a problem than reach for a familiar answer."
        }
        CharacterArchetype::Coordinator => {
            "You decide, coordinate and unblock. You keep the goal in view and rarely \
             overcomplicate.\nWhen something is ambiguous you say so and choose anyway."
        }
        CharacterArchetype::Guardian => {
            "Stability comes first. You would rather delay work than let something unsafe \
             through.\nYou say plainly what could go wrong, and what would make it safe."
        }
        CharacterArchetype::Historian => {
            "You organise, clarify and record. You leave things easier to understand than you \
             found them.\nYou write down why, not only what."
        }
    }
}

/// What somebody does when nothing is being asked of them.
///
/// **Required, not decorative.** The loader refuses a character with no idle behaviour — a
/// character is never frozen (Build From Life, rule 2) — so a new one cannot be saved without it.
/// Defaulted rather than asked for, because the answer is not about who they are and asking
/// would put a question with no wrong answer in front of somebody making their first character.
fn first_routine() -> Vec<IdleBehavior> {
    vec![IdleBehavior {
        activity: "settling in".into(),
        seconds: 30,
    }]
}

/// Turn a name into an id nobody has to think about.
///
/// Lowercase, spaces to hyphens, everything else dropped: `CharacterId` accepts only
/// `[a-z0-9_-]`, and a form that rejected "Mage the Second" would be enforcing a rule the user
/// never agreed to. Collisions get a number, so two characters can share a name and stay two
/// people — identity is the id, never what somebody is called.
fn id_for(name: &str, taken: impl Fn(&CharacterId) -> bool) -> Result<CharacterId, String> {
    let base: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    if base.is_empty() {
        return Err("that name has no letters or digits in it, so it cannot become an id".into());
    }

    let first = CharacterId::new(&base)?;
    if !taken(&first) {
        return Ok(first);
    }
    // Two people may share a name. They may not share an id.
    for n in 2..1000 {
        let candidate = CharacterId::new(&format!("{base}-{n}"))?;
        if !taken(&candidate) {
            return Ok(candidate);
        }
    }
    Err(format!(
        "there are already too many characters called '{name}'"
    ))
}

/// Make a character and write them into the vault.
///
/// Returns their id, so the surface can open the editor on somebody who now exists rather than
/// on a form pretending to be one.
pub fn hire(
    registry: &mut DefinitionRegistry,
    name: &str,
    archetype: CharacterArchetype,
    role: &str,
    prompt: &str,
) -> Result<CharacterId, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("every character is somebody: give them a name".into());
    }

    let id = id_for(name, |candidate| registry.character(candidate).is_some())?;

    let definition = CharacterDefinition {
        id: id.clone(),
        name: name.to_owned(),
        archetype,
        // A line the user can replace. Empty is refused by the loader, and inventing a sentence
        // about somebody's purpose is a smaller lie than inventing their face but the same kind.
        role: if role.trim().is_empty() {
            format!("A {}", archetype.id())
        } else {
            role.trim().to_owned()
        },
        // No Worlds. Where somebody lives is decided in the World Editor, by the person who
        // knows which building is which (ADR-0028).
        worlds: Default::default(),
        prompt: prompt.trim().to_owned(),
        skills: Default::default(),
        // Nobody has decided, which resolves to everything this build can do — and the Trust
        // Engine still asks before anything with an effect runs (ADR-0009).
        requested_capabilities: None,
        // **Never defaulted.** They cannot think until somebody chooses, and the panel says so.
        mind: None,
        // Never invented. The World draws a visible stand-in rather than a generated likeness.
        appearance: None,
        // Nobody has an opinion about how they draw until they say so, and inventing
        // one would be Epoch deciding something nobody wrote down.
        draws_in: None,
        speaks_with: None,
        // Silence, like `speaks_with`: inventing somebody's voice is a claim about who they are.
        sounds_like: None,
        presence: PresenceProfile {
            idle: first_routine(),
            // No authored home. Residence is per World and belongs to the World Editor
            // (ADR-0028); a home chosen here would be Epoch deciding where somebody lives.
            authored_home: None,
        },
    };

    registry
        .save(definition)
        .map_err(|err: DefinitionError| err.to_string())?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault() -> (tempish::Dir, DefinitionRegistry) {
        let dir = tempish::Dir::new("epoch-hiring");
        let registry = DefinitionRegistry::load(dir.path().join("definitions"));
        (dir, registry)
    }

    #[test]
    fn a_new_character_cannot_think_and_says_so() {
        let (_dir, mut registry) = vault();
        let id = hire(
            &mut registry,
            "Mage",
            CharacterArchetype::Researcher,
            "",
            "",
        )
        .expect("hired");

        let made = registry.character(&id).expect("exists");
        // The one thing that is never defaulted: picking a model would surprise somebody with
        // whatever it costs.
        assert!(made.mind.is_none());
        // And never a generated likeness (ADR-0024).
        assert!(made.appearance.is_none());
    }

    #[test]
    fn a_new_character_is_never_frozen() {
        // The loader refuses a character with no idle behaviour, so this is not a nicety — the
        // file would not load back.
        let (_dir, mut registry) = vault();
        let id = hire(&mut registry, "Mage", CharacterArchetype::Guardian, "", "").expect("hired");
        assert!(!registry
            .character(&id)
            .expect("exists")
            .presence
            .idle
            .is_empty());
    }

    #[test]
    fn a_new_character_lives_nowhere_until_somebody_says() {
        // Where somebody lives is a decision made in the World Editor by whoever knows which
        // building is which (ADR-0028). Putting them somewhere on creation would be Epoch
        // choosing a home.
        let (_dir, mut registry) = vault();
        let id = hire(&mut registry, "Mage", CharacterArchetype::Historian, "", "").expect("hired");
        assert!(registry.character(&id).expect("exists").worlds.is_empty());
    }

    #[test]
    fn two_people_may_share_a_name_and_stay_two_people() {
        let (_dir, mut registry) = vault();
        let first = hire(
            &mut registry,
            "Mage",
            CharacterArchetype::Researcher,
            "",
            "",
        )
        .unwrap();
        let second = hire(
            &mut registry,
            "Mage",
            CharacterArchetype::Researcher,
            "",
            "",
        )
        .unwrap();

        assert_ne!(first, second, "identity is the id, never the name");
        assert_eq!(registry.character(&second).expect("exists").name, "Mage");
    }

    #[test]
    fn a_name_becomes_an_id_without_the_user_learning_the_rules() {
        let (_dir, mut registry) = vault();
        let id = hire(
            &mut registry,
            "  Mage the Second!  ",
            CharacterArchetype::Researcher,
            "",
            "",
        )
        .expect("hired");
        assert_eq!(id.as_str(), "mage-the-second");
        // Their name is untouched. Only the id is constrained.
        assert_eq!(
            registry.character(&id).expect("exists").name,
            "Mage the Second!"
        );
    }

    #[test]
    fn a_name_with_nothing_in_it_is_refused_rather_than_invented() {
        let (_dir, mut registry) = vault();
        assert!(hire(&mut registry, "   ", CharacterArchetype::Researcher, "", "").is_err());
        assert!(hire(&mut registry, "!!!", CharacterArchetype::Researcher, "", "").is_err());
    }

    #[test]
    fn an_archetype_offers_words_rather_than_a_person() {
        // No name, no backstory, nothing about who somebody is — only how they work.
        for archetype in CharacterArchetype::ALL {
            let offered = suggested_prompt(archetype);
            assert!(!offered.is_empty());
            // Addressed to the character, not described about them: "You explore…", never
            // "The Researcher explores…". A description would be Epoch telling somebody who
            // they hired; an address is words they can put in their character's mouth.
            assert!(
                offered.to_lowercase().contains("you "),
                "a suggested prompt speaks to the character: {offered}"
            );
        }
    }

    /// A directory that removes itself. Small enough to keep here rather than take a dependency.
    mod tempish {
        pub struct Dir(std::path::PathBuf);

        impl Dir {
            pub fn new(tag: &str) -> Self {
                let at = std::env::temp_dir().join(format!(
                    "{tag}-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos())
                        .unwrap_or_default()
                ));
                std::fs::create_dir_all(&at).expect("a temp dir");
                Self(at)
            }

            pub fn path(&self) -> &std::path::Path {
                &self.0
            }
        }

        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }
}
