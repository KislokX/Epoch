//! How far one model goes on this machine.
//!
//! ## Why this is a separate phase
//!
//! [`crate::card::standard`] runs every model the same way, which is the only thing that makes
//! two numbers comparable. This asks a different question — *how much context can this one hold
//! here* — and the answer is different for every model, so its results never share a table with
//! Standard's. A row where one model ran at 128K and another at 32K compares nothing.
//!
//! ## Loading is not the wall, and that is the whole difficulty
//!
//! Measured on this machine, `gemma4:12b` at 65,536 tokens with a full-precision cache: **9.11 GB
//! reported, of which only 3.53 GB was on the card.** It loaded. It answered. It was running
//! across the PCIe bus at a fraction of its speed, and nothing in the request said so.
//!
//! So a limit search that only asked *did it load* would report a number nobody can use. Three
//! outcomes are kept apart:
//!
//! - **Held** — it loaded and stayed on the card. This is the number worth quoting.
//! - **Spilled** — it loaded, and part of it is in system memory. True, usable, and slow; the
//!   speed is reported beside it so the trade is visible rather than hidden.
//! - **Refused** — it did not load, in the server's own words.
//!
//! ## A ladder, not a binary search
//!
//! Each step is a real model load — tens of seconds — so the count matters. Powers of two from
//! 8K to the model's trained maximum is at most six steps, every one of which is a row somebody
//! can read. A binary search would take fewer and produce a single number with no shape: knowing
//! that 64K holds and 128K spills is worth more than knowing the boundary is somewhere between.

use serde::{Deserialize, Serialize};

/// What happened at one context size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Held {
    /// Loaded, and all of it on the card.
    OnTheCard,
    /// Loaded, and part of it in system memory. Works; slowly.
    Spilled,
    /// Did not load.
    Refused,
}

/// One rung of the ladder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rung {
    pub context: u32,
    pub held: Held,
    /// Tokens per second there. `None` where it did not load.
    pub generation: Option<f64>,
    /// What was on the card while it ran, in bytes.
    pub vram_used: Option<u64>,
    /// The server's own words, where it refused.
    pub note: Option<String>,
}

/// Every rung tried, and what the answer is.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reached {
    pub model: String,
    pub rungs: Vec<Rung>,
}

impl Reached {
    /// The largest context that stayed on the card.
    ///
    /// **This is the number worth quoting**, and it is deliberately not *the largest that
    /// loaded*: one of those is a model running at full speed and the other is a model running
    /// across a bus, and a benchmark that reported the second would be selling somebody a
    /// configuration they would hate.
    pub fn on_the_card(&self) -> Option<u32> {
        self.rungs
            .iter()
            .filter(|it| it.held == Held::OnTheCard)
            .map(|it| it.context)
            .max()
    }

    /// The largest that loaded at all, however slowly.
    pub fn at_all(&self) -> Option<u32> {
        self.rungs
            .iter()
            .filter(|it| it.held != Held::Refused)
            .map(|it| it.context)
            .max()
    }

    /// Why it stopped, in one sentence somebody can act on.
    pub fn why(&self) -> Option<String> {
        let last = self.rungs.last()?;
        Some(match last.held {
            Held::Refused => format!(
                "{} tokens would not load{}",
                last.context,
                last.note
                    .as_deref()
                    .map(|it| format!(": {it}"))
                    .unwrap_or_default()
            ),
            Held::Spilled => format!(
                "{} tokens loaded but spilled into system memory",
                last.context
            ),
            // It reached the top of the ladder without failing, which is the model's own trained
            // limit rather than the machine's.
            Held::OnTheCard => format!(
                "{} tokens held, which is as far as this model was trained",
                last.context
            ),
        })
    }

    /// How much video memory each thousand tokens cost, from two rungs that both held.
    ///
    /// **Derived from two measurements, never from an architecture somebody remembered.** It is
    /// what makes a sentence like *256K would need another four gigabytes* a reading rather than
    /// a guess — and `None` when fewer than two rungs held, because one point is not a slope.
    pub fn per_thousand(&self) -> Option<u64> {
        let mut held: Vec<(u32, u64)> = self
            .rungs
            .iter()
            .filter(|it| it.held == Held::OnTheCard)
            .filter_map(|it| it.vram_used.map(|used| (it.context, used)))
            .collect();
        held.sort_by_key(|(context, _)| *context);
        let (small, low) = *held.first()?;
        let (big, high) = *held.last()?;
        if big <= small || high <= low {
            return None;
        }
        Some((high - low) * 1000 / u64::from(big - small))
    }
}

/// The ladder, up to whatever the model was trained for.
///
/// Powers of two because that is how a context window is spoken about, and because each step
/// doubling means six rungs cover 8K to 256K. Never past the trained maximum: asking for more
/// buys nothing and costs a load to find out.
pub fn ladder(trained: u32) -> Vec<u32> {
    let mut every = Vec::new();
    let mut at = 8_192u32;
    while at <= trained {
        every.push(at);
        let Some(next) = at.checked_mul(2) else { break };
        at = next;
    }
    // The trained maximum itself, when it is not a power of two — a model trained for 262,144
    // stops at 131,072 otherwise, and the last 131,072 tokens are the interesting ones.
    //
    // **And a model trained for less than the first rung gets that rung**, which the first
    // version did not: the loop never ran, the guard required 8,192, and the ladder came back
    // empty — so the limit phase would have said nothing at all about a small model rather than
    // saying it is small. An empty answer to a question that has one is the worst of the three.
    if every.last().is_none_or(|last| *last < trained) && trained > 0 {
        every.push(trained);
    }
    every
}

/// Whether a model that loaded is really on the card.
///
/// **The comparison is against the model, not against an empty card.** A desktop holds a
/// gigabyte or two for its own windows, and a rule that expected an idle card would report every
/// working machine as spilling — a guard whose condition nobody can satisfy, which this codebase
/// has already paid for once.
pub fn stayed(before_free: u64, after_free: u64, weights: u64) -> Held {
    let took = before_free.saturating_sub(after_free);
    // Some of the weights are on the card, and the cache with them. If far less arrived than the
    // weights alone weigh, the rest went somewhere else.
    if took + took / 10 < weights {
        return Held::Spilled;
    }
    Held::OnTheCard
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rung(context: u32, held: Held, vram: Option<u64>) -> Rung {
        Rung {
            context,
            held,
            generation: (held != Held::Refused).then_some(40.0),
            vram_used: vram,
            note: None,
        }
    }

    #[test]
    fn the_ladder_doubles_and_stops_where_the_model_does() {
        assert_eq!(ladder(32_768), vec![8_192, 16_384, 32_768]);
        // A model trained for 262,144 is not a power of two above 131,072, and the last rung is
        // the interesting one.
        let long = ladder(262_144);
        assert_eq!(long.first(), Some(&8_192));
        assert_eq!(long.last(), Some(&262_144));
        assert!(long.len() <= 7, "six or seven loads, not thirty: {long:?}");
        // A model trained for less than the first rung gets one rung, not none.
        assert_eq!(ladder(4_096), vec![4_096]);
    }

    #[test]
    fn the_number_worth_quoting_is_the_one_that_stayed_on_the_card() {
        /*
            Measured on this machine: gemma4:12b at 65,536 reported 9.11 GB with only 3.53 GB
            resident. It loaded, it answered, and it was running across the bus. Quoting that as
            the limit would sell somebody a configuration they would hate.
        */
        let reached = Reached {
            model: "m".to_owned(),
            rungs: vec![
                rung(8_192, Held::OnTheCard, Some(8_000_000_000)),
                rung(16_384, Held::OnTheCard, Some(8_400_000_000)),
                rung(32_768, Held::Spilled, Some(3_500_000_000)),
            ],
        };
        assert_eq!(reached.on_the_card(), Some(16_384));
        assert_eq!(
            reached.at_all(),
            Some(32_768),
            "it did load, and it is not the answer"
        );
        assert!(
            reached.why().is_some_and(|it| it.contains("spilled")),
            "{:?}",
            reached.why()
        );
    }

    #[test]
    fn a_refusal_is_kept_in_the_servers_own_words() {
        let reached = Reached {
            model: "m".to_owned(),
            rungs: vec![
                rung(8_192, Held::OnTheCard, Some(8_000_000_000)),
                Rung {
                    note: Some("failed to allocate KV cache".to_owned()),
                    ..rung(16_384, Held::Refused, None)
                },
            ],
        };
        assert_eq!(reached.on_the_card(), Some(8_192));
        assert_eq!(reached.at_all(), Some(8_192));
        let why = reached.why().expect("it stopped");
        assert!(why.contains("failed to allocate"), "{why}");
    }

    #[test]
    fn a_model_that_reached_its_own_ceiling_says_so_rather_than_the_machines() {
        // Two different facts: *this card ran out* and *the model has no more to give*.
        let reached = Reached {
            model: "m".to_owned(),
            rungs: vec![rung(262_144, Held::OnTheCard, Some(9_000_000_000))],
        };
        let why = reached.why().expect("it finished");
        assert!(why.contains("as far as this model was trained"), "{why}");
    }

    #[test]
    fn the_cost_of_context_is_two_measurements_and_never_one() {
        // One point is not a slope, and a benchmark that extrapolated from one would be quoting
        // an architecture it read somewhere.
        let one = Reached {
            model: "m".to_owned(),
            rungs: vec![rung(8_192, Held::OnTheCard, Some(8_000_000_000))],
        };
        assert_eq!(one.per_thousand(), None);

        // 8K at 8.00 GB and 32K at 8.48 GB: 480 MB over 24,576 tokens.
        let two = Reached {
            model: "m".to_owned(),
            rungs: vec![
                rung(8_192, Held::OnTheCard, Some(8_000_000_000)),
                rung(32_768, Held::OnTheCard, Some(8_480_000_000)),
            ],
        };
        let per = two.per_thousand().expect("two rungs held");
        // ~19.5 MB per thousand tokens.
        assert!((19_000_000..21_000_000).contains(&per), "{per}");
    }

    #[test]
    fn spilling_is_judged_against_the_model_and_not_an_empty_card() {
        /*
            A desktop holds a gigabyte or two for its own windows. A rule that expected an idle
            card would report every working machine as spilling — the guard whose condition
            nobody can satisfy, which this codebase has already paid for once.
        */
        let weights = 7_400_000_000u64;
        // 7.4 GB of weights, and 7.5 GB arrived on the card: held.
        assert_eq!(
            stayed(11_000_000_000, 3_500_000_000, weights),
            Held::OnTheCard
        );
        // Only 3.5 GB arrived: the rest is somewhere else.
        assert_eq!(
            stayed(11_000_000_000, 7_500_000_000, weights),
            Held::Spilled
        );
    }
}
