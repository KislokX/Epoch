//! What it costs to ask this machine which agents it has.
//!
//! ## Why it is asked at all
//!
//! Availability is two facts with two different fixes — installed, and signed in — so Epoch runs
//! the program rather than reading a list. That is what lets a surface say *Claude Code is not
//! installed* instead of simply not offering it, and it is not up for negotiation.
//!
//! What is up for negotiation is **how often**. Four places ask independently: startup, the
//! character editor, the connections panel and the dialogue. Each answer costs one process per
//! agent, plus a second for the ones that are there — nobody has counted what that is, and the
//! roadmap's plan to cache it is a guess until somebody does.
//!
//! ## This measures the machine it runs on
//!
//! An agent that is not installed answers in microseconds — the spawn fails immediately. One
//! that is installed pays for a program starting. So the number is a property of *this* machine
//! and the run prints what it found, because a fast reading from an empty machine would be the
//! most misleading result available.
//!
//! ```text
//! cargo test -p epoch-engine --test the_cost_of_asking -- --ignored --nocapture
//! ```

use std::time::Instant;

/// How many surveys a single pass through the Launcher costs today: startup, the character
/// editor, the connections panel, a dialogue.
const SCREENS: usize = 4;

#[test]
#[ignore = "a measurement of this machine: it runs every agent's own program"]
fn asking_which_agents_this_machine_has_costs_this_much() {
    let agents = epoch_engine::agents::installed();

    // What is actually here, printed — a timing without it says nothing.
    let first = agents.survey();
    for status in &first {
        println!(
            "  {:<12} installed={} signed-in={}",
            status.id,
            status.installed,
            status
                .signed_in
                .map_or("unasked".into(), |yes| yes.to_string())
        );
    }
    let here = first.iter().filter(|s| s.installed).count();

    let began = Instant::now();
    let each = agents.survey();
    let once = began.elapsed();
    assert_eq!(each.len(), first.len(), "the same agents both times");

    println!(
        "\n{} agent(s) known, {here} installed\n  one survey   {once:?}\n  {SCREENS} screens   {:?}",
        first.len(),
        once * SCREENS as u32
    );
}
