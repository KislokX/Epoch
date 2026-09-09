# Phase 1 — Ground truth, and the first person

> **Closed 2026-08-08.** Nine items, eight commits, `b3c9e53 … fb6fb3d`.
> Plan: [ROADMAP](../../../ROADMAP.md) · [Execution Plan](../../Architecture/Execution%20Plan.md)
> Cause: [Full Audit 2026-08-08](../../Architecture/Full%20Audit%202026-08-08.md)

---

## What the phase was for

Two things had to become true before anything else could be done safely: **a change can be
undone**, and **the app knows where it lives**. The Life half was the first person somebody makes.

---

## Done

| | Item | Evidence |
|---|---|---|
| 1.1 | Version control | 268 files, `b3c9e53`, no remote |
| 1.2 | Vitest harness | 27 tests across 5 files |
| 1.3 | IPC contract tests | fixtures written by Rust, claimed by TypeScript |
| 1.4 | One formatting commit | `bff4281`, 601 hunks, no behaviour change |
| 1.5 | MSRV honesty | `1.80` → `1.82` |
| 1.6 | CI | one workflow, six gates |
| 1.7 | `paths.rs` | three locations, resolved once |
| 1.8 | Lazy vault | `Paths::ensure`, nothing created eagerly |
| 1.9 | Create Character | ADD A CHARACTER, from an empty vault |

**Gates, all green:** `cargo fmt --check` clean · `clippy -D warnings` **0** · 421 Rust tests ·
`tsc --noEmit` clean · 27 UI tests · `cargo deny` ok · UI builds.

Tests went from **407 → 448** (421 Rust + 27 UI). The frontend went from **0**.

---

## What the tests found, which is the point

Three defects, none of which any of the 407 existing tests could have caught, because all three
were in layers that had none.

**1. The context gauge kept the previous character's number.** Switching from Mage to Paladin
left Mage's 24,691 on screen. An earlier fix had cleared it on a character *change*; nothing
cleared it for a character with no reading of their own. A reading that describes a conversation
nobody is looking at is worse than no reading — a wrong number is still believed.

**2. `[f32; 2]` and `number[]` are not the same type.** A JSON import widens a fixed-length array,
and the TypeScript side asks for a two-tuple. Narrowed in the test rather than widened in the
type: weakening a real contract to make a test easier is the wrong direction.

**3. `class` was a plain `&'static str` in Rust and a closed union in TypeScript.** They agreed,
and nothing enforced it. A third variant would have produced a value no component handles,
silently. It now throws at the boundary.

---

## What the audit's remediation actually cost

Reaching `clippy -D warnings` required three real changes, not suppressions:

- `DefinitionError::Parse` and `PackError::Parse` **box** their `toml::de::Error`. It was making
  every `Result` in those modules ~150 bytes on the path they almost always take.
- `Token::from_str` became `Token::of`. That name belongs to `std::str::FromStr`, and a method
  wearing it without being it gets called expecting a `Result`.
- MSRV corrected to what the code already required.

`cargo deny` found four things, each **answered rather than silenced**: targets narrowed to
Windows so Tauri's unmaintained GTK3 tree is not vouched for before Linux ships; CDLA-Permissive-
2.0 allowed for Mozilla's root certificate data; the five `unic-*` advisories ignored by id with
the reason and a review date; `publish = false` on the workspace crates, which is simply true.

---

## The one that would have destroyed data

`paths.rs` splits shipped content from user data. The obvious implementation — `shipped` from the
source tree, `data` from `%APPDATA%` — would have been correct for an installed build and
**catastrophic in development**: every character, World and Quest apparently gone on the next
launch, still on disk, with the product saying they did not exist.

So `data` follows the source tree when there is one. `EPOCH_DATA` overrides both, which is how a
test gets a vault of its own instead of writing into somebody's real crew. Asserted by
`where_things_live.rs`, which runs from the same `target/` directory the app does and therefore
measures the real answer rather than a constructed one.

---

## Corrections recorded

- **"Start with a crew of four" was wrong.** It contradicted ADR-0023, and the correction is in
  the code as much as the doc: an archetype may *suggest a prompt*, and may not arrive as a
  person.
- **The first `cargo deny` ignore list was five advisory ids from memory. Every one was wrong.**
  The tool prints them; reading took ten seconds. The same rule that has caught two undocumented
  CLI flags this month applies to a tool's own output.
- **`beforeEach(() => mock.mockReset())`** returns the mock, and Vitest waits on it as a teardown
  — the whole suite timed out at ten seconds. A block body, not an expression.
- **`userEvent.type` does nothing to a range input.** Arrow keys on a slider are the browser's,
  and typing at it fires no event at all, which reads exactly like a broken handler.

---

## One piece of Phase 4 arrived early, deliberately

`ReasoningDial` is extracted from `Dialogue.tsx` (1,222 LOC) because the rule it carries — *a
brain with no measured ladder shows no control* — could not be asserted while it lived inside that
file. Extraction to enable a test is the tests-first rule doing its job, not scope creep. It also
gained an accessible name it did not have: its name was the concatenation `REASONINGAUTO`.

---

## What is now true that was not

- A bad refactor is recoverable.
- A renamed field on either side of the IPC boundary fails in CI instead of appearing as
  `undefined` in a panel.
- Formatting, lints, types, advisories and licences cannot drift silently again.
- The repository is shaped like what a user installs: shipped content and user data are different
  places, and the code knows which is which.
- Somebody with an empty vault can make their first character, and it is **theirs** — no model,
  no face, no home chosen for them.

---

## Next

**Phase 2 — the turn belongs to the Engine.** Its first item is the scripted-agent test, written
against today's code, because that is what makes moving 210 lines out of `state.rs` a refactor
rather than a rewrite.
