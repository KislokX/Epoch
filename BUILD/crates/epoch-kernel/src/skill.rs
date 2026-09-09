//! A way of working, authored once and lent to whoever needs it.
//!
//! ## What a Skill is, and what it deliberately is not
//!
//! A character's `prompt` is **who they are** (CHARACTER_BIBLE): personality, values, how they
//! decide. It travels with them into every World and it is the product.
//!
//! A Skill is **how a job is done**: the steps of a code review, what a good release note
//! contains, the order somebody debugs in. It belongs to nobody, and several characters may be
//! given the same one without becoming the same person.
//!
//! So a Skill may never touch identity. It carries no `temperature`, no model, no face, no
//! archetype — those are behavioural identity and they are the Character's (ADR-0026). A Skill
//! that could set them would be a second, quieter way to author a person, and the two would
//! eventually disagree about who somebody is.
//!
//! ## The second Definition type (ADR-0011)
//!
//! ADR-0011 deferred a generic `Definition` trait until a second type existed. This is it, and
//! the shape it arrived in answers the question rather than the trait: `CharacterDefinition`
//! carries a dozen fields about a person, and this carries five about a method. What the two
//! genuinely share is not a shape — it is the **loader**: read a directory of TOML, insist the id
//! matches the filename, collect problems without failing, and notice a file changing on disk.
//!
//! So the reuse is [`crate::skill`] and `CharacterDefinition` staying separate types over one
//! shared registry, rather than one trait pretending they are the same thing.
//!
//! ## Requirements are requests, not grants
//!
//! `requires` says what the *method* needs — `mcp:playwright`, `read_file`. Naming it does not
//! give it to anybody: a Skill is authored content and cannot widen what a character may do
//! (ADR-0026 — requested, never declared). What it earns is the ability to *say so*, which is
//! what lets a surface show "this Skill wants Playwright and this character does not have it"
//! instead of the Skill quietly failing on its third step.
//!
//! Written as capability **requests** so `mcp:playwright` means the whole server and stays true
//! when that server adds a tool next month — the same reason a character stopped listing two
//! dozen frozen ids (ADR-0008, and the Workshop step that followed it).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::mind::CapabilityRequest;

/// Stable identity of a Skill.
///
/// Same alphabet as a character's, for the same reason: it is a filename in the vault, and
/// validating on construction is what makes every use of it afterwards infallible. Never derived
/// from the name — renaming a Skill must not silently unassign it from everyone who has it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct SkillId(String);

impl SkillId {
    pub fn new(raw: &str) -> Result<Self, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("a skill id cannot be empty".into());
        }
        if let Some(bad) = trimmed
            .chars()
            .find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_' || *c == '-'))
        {
            return Err(format!(
                "skill id '{trimmed}' contains '{bad}'; use lowercase letters, digits, '_' or '-'"
            ));
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SkillId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for SkillId {
    type Error = String;
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        Self::new(&raw)
    }
}

impl<'de> Deserialize<'de> for SkillId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::new(&raw).map_err(serde::de::Error::custom)
    }
}

/// One authored way of working.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillDefinition {
    pub id: SkillId,
    /// What it is called, for a person. Never keyed on.
    pub name: String,
    /// One line: what this is for, shown wherever a Skill is chosen.
    ///
    /// For the reader, not for the model — the model is told the whole method. A list of Skills
    /// with no summaries is a list of filenames somebody has to open one by one.
    #[serde(default)]
    pub summary: String,
    /// The method itself, in the author's words. This is the substance.
    pub method: String,
    /// What the method needs to be carried out.
    ///
    /// A request and never a grant: naming `mcp:playwright` here does not hand it to anybody.
    /// Empty is the ordinary case — most ways of working need nothing in particular.
    #[serde(default)]
    pub requires: BTreeSet<CapabilityRequest>,
}

impl SkillDefinition {
    /// Everything wrong with this Skill, in the author's terms.
    ///
    /// Reported rather than refused, the same way a character's problems are: one bad file must
    /// not stop the others loading, and a Skill with an empty method is a mistake somebody can
    /// see and fix rather than a crash they have to bisect.
    pub fn problems(&self) -> Vec<String> {
        let mut found = Vec::new();
        if self.name.trim().is_empty() {
            found.push(format!("skill '{}' has no name", self.id));
        }
        if self.method.trim().is_empty() {
            // The one field without which the Skill does nothing at all. A character given it
            // would be told they have a way of working, and then told nothing about it.
            found.push(format!(
                "skill '{}' declares no method; a way of working with no steps is nothing to give anybody",
                self.id
            ));
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(method: &str) -> SkillDefinition {
        SkillDefinition {
            id: SkillId::new("code-review").unwrap(),
            name: "Code review".into(),
            summary: "How this project reviews a change.".into(),
            method: method.into(),
            requires: BTreeSet::new(),
        }
    }

    #[test]
    fn an_id_is_a_filename_and_is_checked_when_it_is_made() {
        assert!(SkillId::new("code-review").is_ok());
        assert!(SkillId::new("release_notes").is_ok());
        assert!(SkillId::new("  spaced  ").is_ok(), "trimmed, not rejected");

        assert!(SkillId::new("").is_err());
        // The reason it is constrained at all: it becomes a path in the vault.
        assert!(SkillId::new("../escape").is_err());
        assert!(SkillId::new("Code Review").is_err());
    }

    #[test]
    fn a_skill_with_no_method_says_so_rather_than_being_given_to_somebody() {
        assert!(skill("Read the diff twice.").problems().is_empty());

        let empty = skill("   ");
        assert_eq!(empty.problems().len(), 1);
        assert!(
            empty.problems()[0].contains("no steps"),
            "{:?}",
            empty.problems()
        );
    }

    #[test]
    fn a_skill_names_what_it_needs_as_a_request_and_never_as_a_grant() {
        // `mcp:playwright` rather than two dozen tool ids: the whole server, still true when it
        // adds a tool next month. The same reason a character stopped listing frozen ids.
        //
        // Asserted through JSON rather than the vault's TOML, because the Kernel has no file
        // format and must not grow one — what is being checked here is the *type*, and reading
        // a directory belongs to the Engine's registry, where its own test lives.
        let parsed: SkillDefinition = serde_json::from_value(serde_json::json!({
            "id": "browser-check",
            "name": "Browser check",
            "method": "Open the page and read what is actually rendered.",
            "requires": ["mcp:playwright", "read_file"],
        }))
        .expect("a skill parses");

        assert_eq!(parsed.requires.len(), 2);
        assert!(parsed
            .requires
            .iter()
            .any(|r| r.as_str() == "mcp:playwright"));
        assert!(parsed.summary.is_empty(), "a summary is optional");
    }
}
