//! Looking at a picture somebody shared.
//!
//! The capability half of [`crate::sight`] — that module holds the reasoning; this holds the
//! tool. Two things are worth reading here before the code.
//!
//! ## It only looks at what was shared in this conversation
//!
//! Not a path, not a folder, not "any image on this machine". The parameter is the **name the
//! user typed**, resolved against the pictures actually attached to this Quest. A character
//! cannot look at a file nobody offered it, and there is no argument it could send that would
//! reach one — the map is built per turn from the Chronicle and there is no other route.
//!
//! That is the same shape as [`crate::capabilities::notes`] and the project readers: confinement
//! by construction rather than by validation.
//!
//! ## What comes back is a claim, and it is labelled as one
//!
//! A description is what a model said about some pixels. It can be wrong, and it is wrong in the
//! most expensive way available — fluently, in the voice of an observation. So the answer is
//! attributed in the text itself: which model said it, and that it is a description rather than
//! the image. A reasoner that reads "gemma4:12b describes it as…" can hedge; one that reads a
//! bare paragraph cannot tell it apart from something it saw.

use std::collections::BTreeMap;
use std::path::PathBuf;

use epoch_kernel::{Arguments, CapabilityId, Descriptor, Explanation, Parameter, ValueKind};

use crate::capability::{Capability, CapabilityError, Outcome};
use crate::sight::Sight;

fn id(raw: &str) -> CapabilityId {
    CapabilityId::new(raw).expect("capability ids in this file are literals")
}

pub fn see_image() -> Descriptor {
    Descriptor::observing(
        id("see_image"),
        "Look at an image the user shared in this conversation and get a description in words. \
         You write what to look for, so ask for exactly what you need — and ask again with a \
         sharper question if the answer is not enough.",
    )
    .taking([
        Parameter::required(
            "image",
            ValueKind::Text,
            "Which picture, by the name it was shared under.",
        ),
        Parameter::required(
            "look_for",
            ValueKind::Text,
            "What to look for, in plain words. `Describe this image` gets a vague paragraph; \
             `What is the exact error text in the red box?` gets the error text.",
        ),
    ])
}

/// Look at a picture shared in this conversation.
pub struct SeeImage {
    /// Where shared images live. Owned by the Engine; never supplied by a caller.
    images: PathBuf,
    /// What the user called each picture, and what Epoch called the file. Built per turn from
    /// this Quest, which is what makes "only what was shared here" structural.
    shared: BTreeMap<String, String>,
    eyes: Box<dyn Sight>,
}

impl SeeImage {
    pub fn new(
        images: impl Into<PathBuf>,
        shared: BTreeMap<String, String>,
        eyes: Box<dyn Sight>,
    ) -> Self {
        Self {
            images: images.into(),
            shared,
            eyes,
        }
    }

    /// Which file the user means, matched the way a person would say it.
    ///
    /// Case-insensitively, because somebody typing `error.png` about `Error.png` is not making a
    /// mistake worth a refusal. Never a prefix match: two screenshots called `error.png` and
    /// `error-2.png` would silently become one picture, and a description of the wrong image is
    /// worse than a refusal — it is confidently about something else.
    fn find(&self, asked: &str) -> Option<&str> {
        let wanted = asked.trim().to_lowercase();
        self.shared
            .iter()
            .find(|(name, _)| name.to_lowercase() == wanted)
            .map(|(_, file)| file.as_str())
    }

    /// Everything that was shared here, for a refusal that names the alternatives.
    fn offered(&self) -> String {
        if self.shared.is_empty() {
            return "nothing has been shared in this conversation".into();
        }
        format!(
            "shared here: {}",
            self.shared.keys().cloned().collect::<Vec<_>>().join(", ")
        )
    }
}

impl Capability for SeeImage {
    fn describe(&self) -> Descriptor {
        see_image()
    }

    fn explain(&self, arguments: &Arguments) -> Result<Explanation, CapabilityError> {
        let descriptor = self.describe();
        let problems = arguments.check(&descriptor.parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        let image = arguments
            .text("image")
            .map_err(CapabilityError::BadArguments)?;
        let look_for = arguments
            .text("look_for")
            .map_err(CapabilityError::BadArguments)?;
        Ok(Explanation::of(
            &descriptor,
            format!("look at `{image}` for: {look_for}"),
        ))
    }

    fn run(&self, arguments: &Arguments) -> Result<Outcome, CapabilityError> {
        let problems = arguments.check(&self.describe().parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        let asked = arguments
            .text("image")
            .map_err(CapabilityError::BadArguments)?;
        let look_for = arguments
            .text("look_for")
            .map_err(CapabilityError::BadArguments)?;
        if look_for.trim().is_empty() {
            return Err(CapabilityError::BadArguments(
                "say what to look for — a blank question gets a vague paragraph".into(),
            ));
        }

        // Refused before anything is loaded, and named: "no model here can see" is something the
        // user can act on, and it is not the same failure as "that picture is not here".
        let Some(who) = self.eyes.who() else {
            return Err(CapabilityError::Failed(
                "no model on this machine can see images. Install one that declares vision — \
                 Epoch asks each model rather than keeping a list."
                    .into(),
            ));
        };

        let Some(file) = self.find(asked) else {
            return Err(CapabilityError::Failed(format!(
                "no image called '{asked}' was shared in this conversation ({})",
                self.offered()
            )));
        };

        let bytes = std::fs::read(self.images.join(file)).map_err(|err| {
            CapabilityError::Failed(format!("'{asked}' is no longer in the vault: {err}"))
        })?;

        let described = self
            .eyes
            .look(&bytes, look_for)
            .map_err(CapabilityError::Failed)?;

        // Attribution, and nothing else.
        //
        // This used to add "ask again with a sharper question if it does not answer what you
        // needed" — and measured against a real turn, that reads as an instruction to call
        // again. `gemma4:12b` did exactly as it was told: seven calls, rounds exhausted, no
        // answer at all. The encouragement belongs in the *descriptor*, read once while
        // deciding whether to look, rather than in every result, where it reads as "not
        // finished yet".
        //
        // What stays is the attribution, because it is what keeps a description from being
        // mistaken for the image: a claim by a named model, not something this character saw.
        Ok(Outcome::told(format!(
            "{who} describes '{asked}' as: {}",
            described.trim()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eyes that answer without a model, so what this file *does* is testable.
    struct Fake {
        who: Option<String>,
        says: String,
    }

    impl Sight for Fake {
        fn who(&self) -> Option<String> {
            self.who.clone()
        }
        fn look(&self, _image: &[u8], look_for: &str) -> Result<String, String> {
            Ok(format!("{} (asked: {look_for})", self.says))
        }
    }

    /// A scratch folder **per test**, because they run in parallel.
    ///
    /// One shared folder passed alone and failed together: each test removed it on the way out
    /// while the others were still reading. A test that fails depending on who else is running
    /// teaches people to re-run instead of to look.
    fn shared(name: &str) -> (PathBuf, BTreeMap<String, String>) {
        let dir = std::env::temp_dir().join(format!("epoch-see-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        std::fs::write(dir.join("aaaa.png"), b"pixels").expect("write");
        let mut map = BTreeMap::new();
        map.insert("Error.png".to_owned(), "aaaa.png".to_owned());
        map.insert("error-2.png".to_owned(), "bbbb.png".to_owned());
        (dir, map)
    }

    fn asked(image: &str, look_for: &str) -> Arguments {
        Arguments::new()
            .with("image", epoch_kernel::Value::Text(image.into()))
            .with("look_for", epoch_kernel::Value::Text(look_for.into()))
    }

    fn eyes(who: Option<&str>) -> Box<dyn Sight> {
        Box::new(Fake {
            who: who.map(str::to_owned),
            says: "a blue square".into(),
        })
    }

    #[test]
    fn a_description_arrives_attributed_as_a_claim() {
        // The failure this prevents is fluent and expensive: a description is what a model said
        // about some pixels, and a reasoner that cannot tell it from an observation will repeat
        // it as one.
        let (dir, map) = shared("claim");
        let seen = SeeImage::new(&dir, map, eyes(Some("gemma4:12b")))
            .run(&asked("Error.png", "what colour is it?"))
            .expect("looked");

        // Named, so a reasoner can tell a claim from something it saw. Short, because the
        // longer version told the model to ask again and it obeyed until the rounds ran out.
        assert!(seen.content.contains("gemma4:12b describes"));
        assert!(seen.content.contains("a blue square"));
        assert!(
            !seen.content.contains("ask again"),
            "a result must not read as an instruction to call again: {}",
            seen.content
        );
        // The caller's own question reached the translator — the point of the whole design.
        assert!(seen.content.contains("asked: what colour is it?"));
        // Looking leaves nothing behind, so it is never evidence (ADR-0025).
        assert!(seen.evidence.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_what_was_shared_in_this_conversation_can_be_looked_at() {
        // Confinement by construction: the map is built per turn from the Chronicle, so there is
        // no argument that reaches a file nobody offered.
        let (dir, map) = shared("confined");
        let refused = SeeImage::new(&dir, map, eyes(Some("gemma4:12b")))
            .run(&asked("C:/Users/someone/private.png", "what is it?"));

        let Err(CapabilityError::Failed(why)) = refused else {
            panic!("a picture nobody shared must not be readable");
        };
        // And the refusal names what *was* shared, so the next attempt is informed.
        assert!(why.contains("shared here: Error.png, error-2.png"), "{why}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_name_is_matched_whole_and_never_as_a_prefix() {
        // `error.png` and `error-2.png` are two screenshots. A prefix match would make them one,
        // and a confident description of the wrong picture is worse than a refusal.
        let (dir, map) = shared("prefix");
        let seeing = SeeImage::new(&dir, map, eyes(Some("gemma4:12b")));

        // Case is forgiven — somebody typing `error.png` about `Error.png` is not mistaken.
        assert!(seeing.find("error.png").is_some());
        // The other one resolves to its own file, not to this one.
        assert_eq!(seeing.find("error-2.png"), Some("bbbb.png"));
        assert!(seeing.find("error").is_none(), "no prefix matching");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_machine_that_cannot_see_says_so_and_says_what_to_do() {
        // Refused *before* the picture is even looked up, because "no model here can see" and
        // "that picture is not here" are different problems with different fixes.
        let (dir, map) = shared("blind");
        let refused = SeeImage::new(&dir, map, eyes(None)).run(&asked("Error.png", "what?"));

        let Err(CapabilityError::Failed(why)) = refused else {
            panic!("a blind machine must refuse");
        };
        assert!(why.contains("no model on this machine can see"));
        assert!(why.contains("declares vision"), "and how to fix it: {why}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
