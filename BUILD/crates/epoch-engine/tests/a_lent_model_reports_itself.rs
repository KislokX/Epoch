//! Does a lent model's own description survive the crossing?
//!
//! ## What this exists for
//!
//! A Bridge never implemented `surface`, so it took the trait's silent default and a paired
//! machine's models were drawn with **no capabilities at all**. Not measured and refused —
//! never asked, and an empty row reads as *it cannot see*. The unit tests assert the shape of
//! `Shown` and the parsing of `/api/show`; neither of them would have caught the absence,
//! because the absence was a method nobody wrote.
//!
//! So this asks the only question that settles it: put the same model on two machines and see
//! whether both answers agree.
//!
//! ## What it measured, 2026-08-21
//!
//! ```text
//! bridge / gemma4:12b   128ms   window 262144   sees ✓ hears ✓ tools ✓ thinks ✓
//! ollama / gemma4:12b    30ms   window 262144   sees ✓ hears ✓ tools ✓ thinks ✓
//!
//! bridge / gemma4:26b   263ms   window 262144   sees ✓ hears ✗ tools ✓ thinks ✓
//! ollama / gemma4:26b    17ms   window 262144   sees ✓ hears ✗ tools ✓ thinks ✓
//!
//! ollama / qwen3:14b    170ms   window  40960   sees ✗ hears ✗ tools ✓ thinks ✓
//! ```
//!
//! The `26b` row is the one that carries weight: it differs from `12b` in exactly one field,
//! **and the difference survives the network**. A constant, a default or a guess would have made
//! the two identical. `qwen3:14b` is the contrast — a model that says it cannot see, said so.
//!
//! One deliberate difference remains: a Bridge reports **no controls**. `num_ctx` is a knob on a
//! runtime somebody else owns, and the canonical parameters (ADR-0026) already travel inside the
//! turn.
//!
//! ## Why it is `#[ignore]`d
//!
//! It needs a paired machine that is switched on, holding a named model, and it reaches the
//! network. Run deliberately:
//!
//! ```text
//! cargo test -p epoch-engine --test a_lent_model_reports_itself -- --ignored --nocapture
//! ```

/// Which model to ask about on both machines.
///
/// One that **declares vision**, on purpose: this file exists because vision was the capability
/// whose absence was visible, and a model with nothing interesting to say would prove nothing.
const ON_BOTH: &str = "gemma4:12b";

#[test]
#[ignore = "needs a paired machine holding the model, and reaches the network"]
fn a_lent_model_says_the_same_thing_as_the_one_here() {
    let vault = epoch_engine::paths::Paths::discover().vault();
    let secrets = epoch_engine::secrets::Secrets::at(&vault);
    let backends = epoch_engine::backends::Backends::load(&vault);
    let registry = backends.registry(&secrets);

    // The machine's *default* Provider — its Ollama. The named ones address llama.cpp and LM
    // Studio, and neither describes its models at all, which is a different (and also correct)
    // answer this test would misread as a failure.
    let Some(bridge) = backends
        .bridges
        .iter()
        .find(|id| id.matches(':').count() == 1)
    else {
        panic!("no paired machine — pair one under MACHINES first");
    };

    let there = registry
        .get(bridge)
        .expect("the roster and the registry agree")
        .surface(ON_BOTH);
    let here = registry
        .get("ollama")
        .expect("this machine's own Ollama")
        .surface(ON_BOTH);

    println!("there: {there:?}");
    println!("here:  {here:?}");

    // **Unasked is not "no", and this is the assertion that would have failed before.** A Bridge
    // with no `surface` answered `None` here, and the editor drew an empty row.
    let there_can = there.can.expect("the lent machine described its model");
    let here_can = here.can.expect("this machine described its model");

    assert_eq!(
        there_can, here_can,
        "the same model on two machines described itself differently"
    );
    assert!(
        there_can.sees,
        "{ON_BOTH} declares vision, and it has to cross"
    );
    assert_eq!(there.window, here.window, "and so does how much it holds");

    // The one difference that is deliberate: `num_ctx` is a knob on somebody else's runtime.
    assert!(
        there.controls.is_empty(),
        "a Bridge offers no knobs on a machine it does not own"
    );
    assert!(!here.controls.is_empty(), "this machine's own does");
}
