//! Making a picture.
//!
//! The mirror of [`crate::capabilities::see`]: that one turns pixels into words, this one turns
//! words into pixels. A Provider answers a Conversation; a character does not think in pictures,
//! it asks for one (ADR-0030).
//!
//! ## One thing draws, and it is the panel (2026-08-28)
//!
//! `draw_image` and `edit_image` used to live here. A character asked for a picture guessed a
//! description, a Style, a size and an effort from one sentence, and drew. They are gone. The
//! only drawing capability left is [`open_studio`]: the character opens the Studio Panel and the
//! **person** chooses (ADR-0033).
//!
//! That is not a new rule so much as the old one finally being the only one. ADR-0033 already
//! said the panel is offered before anything is drawn, and two capabilities answering the same
//! intent meant a model could pick the one that skipped the person — which it did.
//!
//! What each of them was defending is kept, and cheaper:
//!
//! - **A character may not name a workflow, a checkpoint, a sampler or a step count.** It cannot
//!   name anything at all now: `open_studio` takes no arguments. The rule ADR-0016 has held
//!   since the Asset Resolver — concepts, never filenames — is true by construction rather than
//!   by a descriptor being carefully worded.
//! - **Nothing is substituted.** Asked for a Style nothing here can serve, `draw_image` had to
//!   refuse without the refusal reading like something to talk over. There is no refusal to word:
//!   the panel shows what is installed.
//! - **A style is never invented.** `gemma4:12b` once passed `style: anime` nobody had asked for
//!   and then announced a picture that did not exist. A model that passes no arguments invents
//!   none.
//!
//! Codex is untouched. It draws with its own `image_gen` and Epoch witnesses the result rather
//! than authorising it — Epoch does not govern an agent's own tools.
//!
//! ## What is left below the capability
//!
//! [`Easel`], [`Asked`], [`Shape`] and [`Detail`] outlived the capability by one consumer: the
//! Connections deck's chain test. See the note above them.

use epoch_kernel::{Arguments, CapabilityId, Descriptor, Effect, Explanation, Reversal};

use crate::capability::{Capability, CapabilityError, Outcome};

fn id(raw: &str) -> CapabilityId {
    CapabilityId::new(raw).expect("capability ids in this file are literals")
}

/// Where a finished picture is, and what made it.
#[derive(Debug, Clone, PartialEq)]
pub struct Drawn {
    /// The file, by the name the World will show it under.
    pub file: String,
    /// Which Style actually served it — so the answer can say, and never imply another.
    pub style: String,
    /// How long it took, for a sentence that does not pretend it was instant.
    pub seconds: f32,
}

/// Opening the Studio Panel in the conversation (ADR-0033).
///
/// ## Why this is a capability and not a button
///
/// A button is something the user has to know about. This is the character **answering**: asked
/// for a picture without being told how it should look, it opens the panel as its reply, the way
/// a person would say *sure — tell me how you want it*.
///
/// That is also what keeps `draw_image` honest. A character guessing twelve decisions from one
/// sentence will guess some of them wrong, and the only correction available was another
/// sentence. Now it can hand the decisions back.
///
/// ## It makes nothing
///
/// `Effect::Reads` and nothing else: no file, no picture, nothing to undo. What it changes is
/// what the window is showing, which is why the surface supplies the doing of it — the Engine
/// owns whether it may happen and the surface owns the opening (ADR-0003).
pub fn open_studio() -> Descriptor {
    Descriptor::acting(
        id("open_studio"),
        // **Every medium the panel makes, named.** It said *picture panel*, and a person asking
        // for a sound got a Spotify search instead — measured in the window, 2026-08-28. A
        // descriptor is the only instruction a model gets about when to reach for this, and one
        // that names one of three media is one a model will reach for a third of the time.
        //
        // Still no style, no size and no length: what it opens is a form, and everything in the
        // form is the person's (ADR-0033).
        "Open the studio panel in this conversation so the user can make a picture, a video, a \
         sound or a 3D model — they choose the model, any LoRAs, the size or length, and the \
         prompt themselves.",
        [Effect::Reads],
        Reversal::NothingToUndo,
    )
    .taking([])
}

/// Whatever can actually put the panel on the screen.
///
/// A trait for the same reason `Easel` is one: the Engine decides that it may open and says so
/// in the Chronicle; the surface knows what a window is.
pub trait Opens: Send + Sync {
    fn open(&self) -> Result<(), String>;

    /// Whether the person has already said how they want pictures drawn.
    ///
    /// **This is what makes the offer reliable rather than a hint.** A descriptor asking a model
    /// to prefer one tool is a suggestion, and a small model takes the first tool that matches:
    /// measured 2026-08-24, `gemma4:12b` answered *"haz una imagen de asuka"* by inventing a
    /// whole prompt of its own and calling `draw_image`. Which is precisely what the panel
    /// exists to stop.
    ///
    /// So the Runtime decides, not the character — `CLAUDE.md`'s own rule. Nothing chosen yet
    /// means the panel is the answer, whatever tool was reached for.
    fn chosen(&self) -> bool;
}

/// The capability itself.
pub struct OpenStudio {
    surface: Box<dyn Opens>,
}

impl OpenStudio {
    pub fn new(surface: Box<dyn Opens>) -> Self {
        Self { surface }
    }
}

impl Capability for OpenStudio {
    fn describe(&self) -> Descriptor {
        open_studio()
    }

    fn explain(&self, _arguments: &Arguments) -> Result<Explanation, CapabilityError> {
        Ok(Explanation::of(
            &self.describe(),
            "open the creations panel so you can fill it in".to_owned(),
        ))
    }

    fn run(&self, _arguments: &Arguments) -> Result<Outcome, CapabilityError> {
        self.surface.open().map_err(|why| {
            CapabilityError::Failed(format!("the panel could not be opened: {why}"))
        })?;
        // **Bounded, and it names what not to do.** ADR-0030's third amendment: what a tool says
        // back is prompt, and a weak model completes a status report by narrating past it. This
        // one ends the turn on purpose — there is nothing to add until the user has pressed
        // Generate, and a character that described a picture here would be describing one that
        // does not exist.
        Ok(Outcome::told(PANEL_IS_OPEN))
    }
}

/// What a character is told once the panel is up.
///
/// Bounded and negative, per ADR-0030's third amendment: a model handed a status report completes
/// it, and the completion here would be a picture that does not exist.
///
/// **One constant, because a model reads this.** The same words were written out twice in two
/// spellings, and this one had been carrying literal `\n` and five spaces of source indentation
/// into the prompt since a repair went wrong. Invisible to every test — nothing asserts on
/// whitespace — and read by the model on every turn that opens the panel.
const PANEL_IS_OPEN: &str = "The panel is now open in this conversation. Tell the user, in one \
     short sentence, to choose what they want and press Generate. Say nothing about how the \
     picture will look and do not describe one — none has been made.";

// ---------------------------------------------------------------------------------------------
// A bench, and what to ask of it. **No longer a capability's surface.**
//
// These were `draw_image`'s arguments. That capability is gone (2026-08-28) and one consumer is
// left: the Connections deck's chain test, which walks Style -> workflow -> compile -> server ->
// kept file and needs a way to draw one picture through the real machinery.
//
// **It is not a shortcut and it is not reachable by a character.** The panel is the only thing a
// person draws with, and the panel does not come through here — it composes its own `Ask` from
// what the person chose (ADR-0033). What survives here is the older, smaller request: a Style,
// which resolves to a workflow somebody attached, plus a size and an effort.
//
// The halves that read a *model's prose* into a `Shape` and a `Detail` went with the capability.
// Nothing parses a model's words into a picture any more, which is the point of the change.
// ---------------------------------------------------------------------------------------------

/// Where a picture is made, whatever is doing the making.
///
/// A trait for the same reason [`crate::sight::Sight`] is one: the caller owns *what was asked
/// for* and something else owns *how it is produced*. That keeps ComfyUI out of this file
/// entirely.
pub trait Easel: Send + Sync {
    /// Which Styles can actually be served here, in the order they are offered.
    ///
    /// Asked rather than stored: somebody attaches a workflow while Epoch is open, and a
    /// remembered answer would be wrong in the direction that matters — claiming a Style this
    /// machine cannot draw.
    fn styles(&self) -> Vec<String>;

    /// Draw it, and answer with where it landed.
    fn draw(&self, asked: &Asked) -> Result<Drawn, String>;
}

/// What was asked for, in the World's own words.
#[derive(Debug, Clone, PartialEq)]
pub struct Asked {
    pub describe: String,
    /// `None` means *whatever this World usually draws with*.
    pub style: Option<String>,
    pub shape: Shape,
    pub detail: Detail,
    /// A picture to work from, as a path the Engine resolved.
    pub from_image: Option<std::path::PathBuf>,
}

/// The shape of the picture, as a person would say it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shape {
    #[default]
    Square,
    Portrait,
    Wide,
    /// Whatever the picture being changed already is.
    Keep,
}

impl Shape {
    /// Pixels, at the size the common models are trained for.
    ///
    /// Not arbitrary: SDXL is trained at 1024×1024 and its own template lists the sizes that
    /// work — 1216×832 and 832×1216 among them. Asking for 1920×1080 gets a worse picture, not
    /// a wider one, so the shape is a shape and the numbers belong here.
    pub const fn pixels(self) -> Option<(u32, u32)> {
        match self {
            Shape::Square => Some((1024, 1024)),
            Shape::Portrait => Some((832, 1216)),
            Shape::Wide => Some((1216, 832)),
            // Nothing to set: the picture decides.
            Shape::Keep => None,
        }
    }
}

/// How hard to work at it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Detail {
    Quick,
    #[default]
    Normal,
    Fine,
}

impl Detail {
    /// A multiplier on whatever the workflow already asks for.
    ///
    /// **Relative, never absolute.** A workflow's author chose 20 steps or 8 steps for a reason —
    /// a turbo model needs four — and replacing that with a number of Epoch's own would make
    /// *fine* mean *broken* on half of them.
    pub const fn times(self) -> f32 {
        match self {
            Detail::Quick => 0.5,
            Detail::Normal => 1.0,
            Detail::Fine => 1.75,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_the_panel_never_describes_a_picture() {
        // ADR-0030's third amendment: what a tool says back is prompt. A model handed a status
        // report will complete it, and the completion here would be a picture that does not
        // exist — exactly the failure that ended a conversation in a lie once already.
        struct Opened;
        impl Opens for Opened {
            fn open(&self) -> Result<(), String> {
                Ok(())
            }
            fn chosen(&self) -> bool {
                false
            }
        }
        let said = match OpenStudio::new(Box::new(Opened)).run(&Arguments::new()) {
            Ok(outcome) => format!("{outcome:?}"),
            other => panic!("{other:?}"),
        };
        assert!(said.contains("open"), "{said}");
        assert!(said.contains("none has been made"), "{said}");
        for forbidden in ["here is", "i made", "looks like"] {
            assert!(!said.to_lowercase().contains(forbidden), "{said}");
        }
    }

    #[test]
    fn opening_the_panel_makes_nothing_and_says_so() {
        // `Reads`, and nothing to undo, because nothing was written. Claiming `Writes` would ask
        // for an approval about a file that does not exist.
        let descriptor = open_studio();
        assert!(
            descriptor.effects.contains(&Effect::Reads),
            "{descriptor:?}"
        );
        assert!(
            !descriptor.effects.contains(&Effect::Writes),
            "{descriptor:?}"
        );
    }

    #[test]
    fn a_surface_that_cannot_open_it_fails_rather_than_pretending() {
        struct Shut;
        impl Opens for Shut {
            fn open(&self) -> Result<(), String> {
                Err("there is no window open yet".to_owned())
            }
            fn chosen(&self) -> bool {
                false
            }
        }
        let why = OpenStudio::new(Box::new(Shut))
            .run(&Arguments::new())
            .unwrap_err();
        assert!(format!("{why:?}").contains("no window"), "{why:?}");
    }
}
