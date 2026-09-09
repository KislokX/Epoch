# Retrospective — Milestone 1, Step 1: "The World exists"

> Closed 2026-07-26. Based on implementation evidence only.
> Practice: [[../Build From Life]] rule 16.

## Delivered

A Cargo workspace (`epoch-kernel`, `epoch-engine`, `epoch-tauri`), a React presentation layer, and the default World Pack. The app opens directly into the World; the five places are named by the pack, never by code. 12 tests green; frontend typechecks and builds.

## 1. Assumptions validated

**ADR-0003 (Engine / Presentation separation) — strongest evidence of the step.** The frontend runs standalone in a plain browser with no engine at all and stays coherent: it renders the World and reports honestly that the engine is not answering. The separation proved itself by accident. Exactly one file in the UI knows Tauri exists (`ui/src/ipc/world.ts`).

**ADR-0016 / ADR-0017 (concepts, not names).** The engine references `PlaceConcept`; the pack supplies labels. Enforced by `the_engine_never_leaks_a_concept_id_as_a_label`, which fails if anything bypasses the pack.

**Build From Life rule 1 (the first frame is the World).** Verified in the browser with no engine: sky, ground and status render; zero places, because the UI does not know any and must not invent them.

**"The Kernel depends on nothing."** Now compiler-enforced rather than discipline-enforced: `epoch-kernel` has exactly one dependency (`serde`).

**ADR-0002 (Rust).** Cold workspace build including all of Tauri: ~16 s. Hot rebuild of the shell: 2.3 s. No evidence against the compiled-core decision.

## 2. Assumptions that were incorrect

**MSVC Build Tools were reported missing when they were present.** My check probed `C:\Program Files\Microsoft Visual Studio` paths and found nothing. `tauri info` later reported *Build Tools 2026* and *VS Community 2022* installed. The tooling's own detection was right; mine was wrong. Lesson: use the ecosystem's diagnostic before asserting an environment gap.

**`beforeDevCommand` runs from the crate directory.** It does not. Tauri runs it from the **parent of the config directory** (the `src-tauri/` convention), so `../../ui` resolved one level too high. Found by running it, not by reading docs.

**`tauri dev --config <path>` can be run from anywhere.** It cannot. The CLI requires `tauri.conf.json` to be discoverable from the current folder or a subfolder, independently of `--config`.

**A stale shell PATH looks identical to a missing toolchain.** After rustup was installed, Bash still could not find `cargo` — `~/.cargo/bin` was in the persistent user PATH but not in this session's process PATH. Distinguish "not installed" from "not on this process's PATH" before concluding.

## 3. Simpler than expected

**The fallback chain.** ADR-0016 describes active → base → default → placeholder, which sounded like it needed inheritance or a tree. An ordered `Vec` with first-match and a terminal placeholder satisfies the contract completely, in ~15 lines.

**Pack coverage.** ADR-0016 asks for a queryable coverage Profile. A single `coverage() -> f32` derived from the known vocabulary answers "this pack covers 80%" — no Profile type needed yet.

**"Never a loading screen" was free.** Expected UI work; it fell out of the projection always succeeding. `WorldView::project()` cannot fail, so there is never a state where the World cannot be drawn. The rule became a type-level property rather than a UI behaviour.

**camelCase at the IPC boundary.** One serde attribute, no translation layer. Field naming is part of the projection, so the engine did not have to bend.

## 4. Unexpected complexity

**Tauri's implicit path conventions** cost two debugging cycles (findings above). Not architectural — tooling. No ADR implication.

**The Windows resource icon** forced creating a visual asset before the visual-identity milestone exists. Resolved with a deliberately abstract original mark (a horizon under an arc) and an explicit note that it is not Epoch's identity. A real, if small, tension between a tooling requirement and a deferred creative decision.

**Rust ↔ TypeScript contract has no compiler link.** `ui/src/ipc/contracts.ts` is hand-written to mirror the Rust projection. Nothing enforces the match, so `crates/epoch-engine/tests/wire_contract.rs` was added as a manual seam that fails if the shape changes.

## 5. ADRs with stronger evidence

| ADR | Evidence gained |
|---|---|
| **0003** Engine / Presentation | UI ran with zero engine and stayed coherent — separation demonstrated, not asserted |
| **0016** Asset resolution | Fallback chain implemented + tested; the license hard rule is now executable (`a_manifest_without_a_license_is_rejected`) |
| **0017** Archetypes / World Packs | Engine compiled with no character or place name anywhere in it |
| **0002** Rust | Build times acceptable; no friction observed |

No ADR gained evidence *against* it. No amendment justified.

## 6. Documented, not acted on

**Wire-contract drift risk.** One hand-written test guards the Rust↔TS seam. That is sufficient at this size. It becomes a real drift surface as the contract grows (Step 4 adds Conversation, Step 5 adds the Context Report). Generation from Rust (`ts-rs`, `specta`) is the eventual answer. **Not adopting a dependency for a problem we do not yet have** — watch it, and revisit if the seam actually breaks once.

**Pack paths are development-time.** `epoch-tauri` resolves the default pack via `CARGO_MANIFEST_DIR`, which will not exist in an installed build. ADR-0016 never specified an install-time pack location. Belongs to the milestone that ships an installer; recorded so it is not rediscovered as a bug.

**Tooling requirements can force premature content.** The icon case will recur (splash images, installer art, store assets). Standing answer: satisfy the tooling with something deliberately neutral and label it as placeholder, rather than letting a build requirement pull the Official Epoch Universe forward.

## Living Score

> **Why does Epoch feel more alive today than it did yesterday?**

Honestly: **it does not feel alive yet — it feels present.** Yesterday Epoch was documents. Today it is a place that opens, with named locations that came from data rather than code, and that stays honest when its engine is missing.

Nobody lives there. Nothing moves. That is the correct state for Step 1 and no more: this step lays the floor life will stand on. Step 2 puts someone in it; Step 3 is where the World starts moving on its own.

The score is deliberately low, and that is information — not a failure.
