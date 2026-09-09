//! Sight — turning pixels into text, for a reasoner that never touches pixels.
//!
//! ## Sight is a capability, not a property of a model
//!
//! The obvious arrangement is to give a character a multimodal model and send the picture with
//! the turn. It couples the whole product to which backend the user happens to run, costs the
//! VRAM of a vision model on every turn including the ones with no picture in them, and leaves a
//! character on a text-only model simply blind.
//!
//! So: a **translator** model reads the image and answers in words, and the reasoner asks for
//! that through the same door it asks for `read_file`. A blind local model gets sight; nothing
//! about the reasoner changes; the vision model is loaded only when somebody actually looks.
//!
//! ## The design point, and it was measured
//!
//! **The reasoner writes the extraction prompt.** The published version of this technique
//! hardcodes one generic prompt and then names generic prompts as its own biggest failure. As a
//! *capability* that failure is not something to avoid — it is unavailable, because the caller
//! writes the question and can ask again with a better one.
//!
//! That only matters if the question changes the answer, so it was asked. One 64×64 PNG of a
//! single colour, drawn for the probe so that reading is distinguishable from guessing:
//!
//! | prompt | `gemma4:12b` said |
//! |---|---|
//! | *Describe this image.* | "a solid, uniform field of dark navy blue…" |
//! | *What single colour fills this image? Answer with one word.* | "Navy" |
//!
//! Same picture, same model, same temperature. The question is doing work.
//!
//! ## Who can see is asked, never configured
//!
//! Ollama's `/api/show` returns a `capabilities` list, and `vision` is in it — measured on this
//! machine: `gemma4:26b` and `gemma4:12b` have it, `qwen3:14b` and `gpt-oss:20b` do not. So
//! Epoch does not need a "vision model" setting for the user to understand and keep correct. It
//! asks.
//!
//! A setting would also have been a cold instrument nobody could read: *which* of my models can
//! see is exactly the question a person cannot answer from memory.

/// Somebody who can look at a picture and say what is in it.
///
/// A trait rather than a direct call into the provider registry, for the reason the capability
/// below is testable at all: what `see_image` does with an answer is worth asserting without a
/// model, a network or a GPU.
pub trait Sight: Send + Sync {
    /// Which model would answer. `None` when nothing on this machine can see.
    ///
    /// Asked rather than stored, because installing a model is something a user does while Epoch
    /// is open — and a remembered answer would be wrong in the direction that matters, claiming
    /// sight this machine no longer has.
    fn who(&self) -> Option<String>;

    /// Look at `image` and answer `look_for`.
    ///
    /// The bytes, not a path: the caller already resolved which picture this is, and handing a
    /// path down would make a second thing that could read the wrong file.
    fn look(&self, image: &[u8], look_for: &str) -> Result<String, String>;
}

/// Nobody on this machine can see.
///
/// The honest implementation when no installed model declares vision — and it is what makes the
/// capability's refusal a *sentence* rather than an absence. A character that reaches for
/// `see_image` and is told "no model here can see; install one in the Workshop" has learned
/// something; one whose tool silently vanished has not.
pub struct Blind;

impl Sight for Blind {
    fn who(&self) -> Option<String> {
        None
    }

    fn look(&self, _image: &[u8], _look_for: &str) -> Result<String, String> {
        Err("no model on this machine can see images".into())
    }
}

/// Which backend and model should do the looking.
///
/// A **decision**, so it lives in the Engine rather than in the shell, and it is pure so it can
/// be asserted without a network: the caller supplies what each backend reported.
///
/// The rule, and each half of it is a choice worth naming:
///
/// - **local first.** A hosted backend can see too, and sending somebody's screenshot to a third
///   party is a disclosure decision that belongs to them — the one ADR-0025 already named for
///   Project Roots. Choosing a hosted model quietly on the user's behalf would make that decision
///   for them, invisibly, inside a tool call. So local is preferred, and this returns `None`
///   rather than reaching for a hosted one.
/// - **in the order the backend reported.** Ollama lists models in its own order and Epoch does
///   not sort it (see `Ollama::probe`), so this inherits an ordering the user can see rather
///   than inventing one — and picking "the largest" would need a size table that goes stale.
///
/// `None` means nothing on this machine can see, which is a real answer and the one `see_image`
/// turns into a sentence a person can act on.
pub fn who_can_see(
    surveyed: &[crate::provider::ProviderStatus],
    asks: &dyn Fn(&str, &str) -> Option<crate::provider::Declared>,
) -> Option<(String, String)> {
    surveyed
        .iter()
        .filter(|status| status.online && status.local)
        .find_map(|status| {
            status
                .models
                .iter()
                // `None` is **unasked**, not "cannot see". A backend that failed to answer must
                // not be read as a refusal — the same rule the declaration itself follows.
                .find(|model| asks(&status.id, model).is_some_and(|can| can.sees))
                .map(|model| (status.id.clone(), model.clone()))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderStatus;

    fn status(id: &str, local: bool, online: bool, models: &[&str]) -> ProviderStatus {
        ProviderStatus {
            id: id.into(),
            name: id.into(),
            machine: None,
            endpoint: "http://127.0.0.1:11434".into(),
            online,
            local,
            models: models.iter().map(|m| (*m).to_owned()).collect(),
            note: None,
        }
    }

    #[test]
    fn the_first_model_that_says_it_can_see_is_chosen() {
        // Asked, never guessed. On this machine `gemma4` reports vision and `qwen3` does not,
        // which is measured from `/api/show` — so the fake here answers the same question the
        // real one does.
        let sees = |_backend: &str, model: &str| {
            Some(crate::provider::Declared {
                sees: model.starts_with("gemma4"),
                ..Default::default()
            })
        };
        let found = who_can_see(
            &[status("ollama", true, true, &["qwen3:14b", "gemma4:12b"])],
            &sees,
        );

        assert_eq!(found, Some(("ollama".into(), "gemma4:12b".into())));
    }

    #[test]
    fn a_hosted_backend_is_never_chosen_on_the_user_s_behalf() {
        // Sending somebody's screenshot to a third party is their decision (ADR-0025's
        // disclosure rule). Making it quietly, inside a tool call, is the one way it must not
        // happen — so a machine with only a hosted backend reports no sight at all.
        let sees = |_: &str, _: &str| {
            Some(crate::provider::Declared {
                sees: true,
                ..Default::default()
            })
        };
        let found = who_can_see(&[status("anthropic", false, true, &["opus"])], &sees);

        assert_eq!(found, None);
    }

    #[test]
    fn a_backend_that_is_not_answering_is_not_eyes() {
        let sees = |_: &str, _: &str| {
            Some(crate::provider::Declared {
                sees: true,
                ..Default::default()
            })
        };
        let found = who_can_see(&[status("ollama", true, false, &["gemma4:12b"])], &sees);

        assert_eq!(found, None);
    }

    #[test]
    fn a_backend_that_would_not_answer_is_not_read_as_blind() {
        // Unasked is not "no". A probe that timed out must not decide that a model cannot see —
        // the cold-instrument rule, one layer in from the gauges it was written for.
        let silent = |_: &str, _: &str| None;
        let found = who_can_see(&[status("ollama", true, true, &["gemma4:12b"])], &silent);

        // It is still not chosen, because sight was never confirmed — but the reason is that
        // nothing was learned, and `see_image`'s refusal says so rather than calling it blind.
        assert_eq!(found, None);
    }
}
