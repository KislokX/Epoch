//! The Models Workshop, on the machine that will actually run the model.
//!
//! ## Why this program needs one at all
//!
//! It was built without one, and the first real use showed why that was wrong: two models were
//! pulled onto a 16 GB MacBook — `qwen3.8:27b` at 17.7 GB and `gemma4:26b` at 18.0 GB. Neither
//! fits in that machine's memory with the operating system switched off, let alone running. The
//! download succeeded, the disk filled, and nothing said a word until a turn was slow.
//!
//! Epoch's Workshop asks exactly the right question — *will it run here?* — about exactly the
//! wrong machine. The Host has a 12.9 GB graphics card; the machine that has to hold the weights
//! is this one, three metres away, with unified memory and no card at all. **A verdict measured
//! on the Host is worse than no verdict**, because it is confident and it is about somewhere
//! else.
//!
//! ## The same Workshop, not a second one
//!
//! Every reading here comes from `epoch-models`, the crate Epoch's own Workshop uses. That crate
//! exists because of this file: the alternative was a second implementation of weighing,
//! searching and pulling, and two copies of one decision is how a fix lands in one of them.
//!
//! What differs is only the machine passed in, which is the entire point.

use epoch_models::{Machine, Offer};

/// Whether a model would fit in this machine's memory, in words.
///
/// ## Unified memory is not video memory, and neither is a lie
///
/// On a machine with a graphics card, `fits` compares against free VRAM. On an Apple machine
/// there is no VRAM to compare against — the memory is unified — so the comparison is against
/// system memory instead, and it is stated as such rather than dressed up as the same reading.
///
/// **A tenth is left over**, the same margin `Machine::fits` uses: a model that exactly fills the
/// memory it runs in does not run, and a verdict that said otherwise would be technically true
/// and practically wrong.
/// Whether something of this size runs on this machine.
///
/// **The card when there is one, and this machine's memory when there is not.** Extracted
/// because there were two answers: the verdict panel fell back to unified memory on an Apple
/// machine, while the quantisation chips beside it asked only about video memory — which an
/// Apple machine does not have. Every chip on the Mac therefore read *cannot say*, on a page
/// that was simultaneously saying `RUNS HERE` about the same file.
///
/// `None` means nobody could measure anything, which is a different answer from *no*.
pub fn fits(bytes: u64, machine: &Machine) -> Option<bool> {
    if let Some(card) = machine.fits(bytes) {
        return Some(card);
    }
    // The same tenth left over everywhere else: a model that exactly fills the memory it runs
    // in does not run.
    machine
        .ram_total
        .map(|ram| bytes.saturating_add(bytes / 10) <= ram)
}

pub fn verdict(offer: &Offer, machine: &Machine) -> Verdict {
    let Some(bytes) = offer.bytes else {
        return Verdict {
            runs: None,
            said: "This has not been weighed yet.".to_owned(),
        };
    };

    // The card, when there is one — the same question Epoch's Workshop asks.
    //
    // **And a Mac now answers it**, which it did not before: `Machine` learned to ask `sysctl`
    // and `vm_stat` instead of only NVIDIA. So this branch is reached on Apple Silicon too, with
    // a *free* figure rather than the machine's total — a stricter and truer reading, and the
    // same one the Host uses for its card.
    //
    // The words change with the architecture, because they have to. There is no second pool on a
    // unified machine: what does not fit does not run slowly, it does not load.
    if let Some(fits) = machine.fits(bytes) {
        let pool = if machine.unified {
            "unified memory"
        } else {
            "video memory"
        };
        return Verdict {
            runs: Some(fits),
            said: if fits {
                format!("Fits in free {pool}.")
            } else if machine.unified {
                format!(
                    "Larger than this machine's free {pool}. It shares one pool with the system,                      so there is nowhere for the rest to go."
                )
            } else {
                format!("Larger than free {pool}. It will spill into system memory and run slowly.")
            },
        };
    }

    // No card to ask about. On this machine that is not a gap — it is what an Apple machine is,
    // and its memory is the memory a model runs in.
    let Some(ram) = machine.ram_total else {
        return Verdict {
            runs: None,
            said: "This machine's memory could not be read, so nobody can say.".to_owned(),
        };
    };

    let room = bytes.saturating_add(bytes / 10) <= ram;
    Verdict {
        runs: Some(room),
        said: if room {
            format!("Fits in this machine's {} of memory.", gb(ram))
        } else {
            // The sentence somebody needed before pulling 17.7 GB onto a 17.2 GB machine.
            format!(
                "Needs {}, and this machine has {} in total. It will not fit — the system needs \
                 some of that too.",
                gb(bytes),
                gb(ram)
            )
        },
    }
}

pub struct Verdict {
    /// `None` when the question could not be answered, which is never the same as *no*.
    pub runs: Option<bool>,
    pub said: String,
}

pub fn gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1_000_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apple(ram: u64) -> Machine {
        Machine {
            unified: false,
            gpu: None,
            vram_total: None,
            vram_free: None,
            ram_total: Some(ram),
        }
    }

    fn weighed(bytes: u64) -> Offer {
        Offer {
            pullable: true,
            name: "qwen3.8:27b".into(),
            pull: "qwen3.8:27b".into(),
            installed: false,
            bytes: Some(bytes),
            fits: None,
            source: "featured",
            facets: Vec::new(),
            described: false,
        }
    }

    /// The same MacBook, once Epoch learned to ask it about itself.
    ///
    /// **Measured on the owner's M2, 2026-08-25:** `Apple M2`, 17.18 GB of unified memory, 7.10 GB
    /// of it free — cross-checked against the `ram_free` ComfyUI's own torch build reported at the
    /// same moment. Before this, `Machine::measure` asked `nvidia-smi` and nothing else, so a Mac
    /// reported no graphics at all and this file's fallback compared against *total* memory.
    ///
    /// Two things change and both are improvements: the comparison is against what is free, and
    /// the sentence stops describing somewhere to spill into that does not exist.
    #[test]
    fn an_apple_machine_is_weighed_against_its_free_unified_memory_and_told_so() {
        let m2 = Machine {
            gpu: Some("Apple M2".to_owned()),
            vram_total: Some(17_179_869_184),
            vram_free: Some(7_100_000_000),
            ram_total: Some(17_179_869_184),
            unified: true,
        };

        let big = verdict(&weighed(17_700_000_000), &m2);
        assert_eq!(big.runs, Some(false));
        assert!(big.said.contains("unified memory"), "{}", big.said);
        assert!(
            !big.said.contains("spill"),
            "there is nothing to spill into on one pool: {}",
            big.said
        );

        let small = verdict(&weighed(4_000_000_000), &m2);
        assert_eq!(small.runs, Some(true));
        assert!(small.said.contains("unified memory"), "{}", small.said);

        // And a machine with a card of its own keeps the words that are true of it.
        let card = Machine {
            gpu: Some("NVIDIA GeForce RTX 4070 SUPER".to_owned()),
            vram_total: Some(12_900_000_000),
            vram_free: Some(11_000_000_000),
            ram_total: Some(33_900_000_000),
            unified: false,
        };
        assert!(verdict(&weighed(17_700_000_000), &card)
            .said
            .contains("spill into system memory"));
    }

    #[test]
    fn the_model_that_was_already_pulled_would_have_been_refused() {
        // The exact case this file exists for: 17.7 GB onto a 16 GB MacBook, which reports
        // 17.18 GB of unified memory. It was downloaded, it filled the disk, and nothing said
        // anything until a turn was slow.
        let said = verdict(&weighed(17_700_000_000), &apple(17_179_869_184));
        assert_eq!(said.runs, Some(false));
        assert!(said.said.contains("17.7 GB"), "{}", said.said);
        assert!(said.said.contains("17.2 GB"), "{}", said.said);
    }

    #[test]
    fn a_model_with_room_to_spare_is_allowed() {
        let said = verdict(&weighed(5_000_000_000), &apple(17_179_869_184));
        assert_eq!(said.runs, Some(true));
    }

    #[test]
    fn a_tenth_is_left_for_the_machine_to_keep_running() {
        // A model that exactly fills the memory it runs in does not run. The same margin
        // `Machine::fits` uses for a graphics card, applied where the memory is unified.
        let exactly = 16_000_000_000;
        assert_eq!(
            verdict(&weighed(exactly), &apple(exactly)).runs,
            Some(false)
        );
        assert_eq!(
            verdict(&weighed(exactly), &apple(exactly + exactly / 10)).runs,
            Some(true)
        );
    }

    #[test]
    fn a_machine_that_cannot_be_read_says_nothing_rather_than_no() {
        // No reading beats an invented one — and here an invented *no* would stop somebody
        // downloading a model that would have run perfectly well.
        let blind = Machine {
            unified: false,
            gpu: None,
            vram_total: None,
            vram_free: None,
            ram_total: None,
        };
        assert_eq!(verdict(&weighed(5_000_000_000), &blind).runs, None);
    }

    #[test]
    fn an_unweighed_offer_is_not_a_verdict() {
        let mut unknown = weighed(0);
        unknown.bytes = None;
        assert_eq!(verdict(&unknown, &apple(17_179_869_184)).runs, None);
    }

    #[test]
    fn a_card_is_asked_about_before_system_memory() {
        // On a machine that has one, the video memory is the memory that matters — and system
        // memory being plentiful must not overrule a card that is full.
        let card = Machine {
            unified: false,
            gpu: Some("a card".into()),
            vram_total: Some(12_000_000_000),
            vram_free: Some(6_000_000_000),
            ram_total: Some(64_000_000_000),
        };
        assert_eq!(verdict(&weighed(9_000_000_000), &card).runs, Some(false));
    }
}
