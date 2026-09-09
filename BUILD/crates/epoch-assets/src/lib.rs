//! Authored content Epoch reads, understands and runs.
//!
//! ## What belongs here, and the one test that decides
//!
//! **Did a person author this, or did a machine happen to have it?** A ComfyUI workflow, a World
//! Pack, a Character file, a LoRA's card — someone wrote those, they arrive from outside, and
//! reading one means understanding a shape Epoch did not choose. A graphics card's VRAM, an
//! installed runtime, the size of a GGUF on disk — nobody authored those; they are measurements,
//! and they live in `epoch-models` next door.
//!
//! The split is not cosmetic. Authored content is **untrusted input**: it can be malformed, it
//! can be from a newer version, it can name things this machine has never heard of. Every module
//! here therefore owes the same three answers — what it *is*, what it *can do*, and what it
//! *needs* — and owes them derived from the content rather than declared in a manifest, because
//! a capability somebody typed in is one that will eventually lie (ADR-0030).
//!
//! ## Nothing of Epoch's
//!
//! No Kernel, no Engine. What comes out of here is a plain description that the Engine decides
//! what to do with — the same arrangement `epoch-models` has, and what keeps both linkable by
//! EpochServices without dragging Quests and Trust along (ADR-0029 §9).

pub mod asset;
pub mod workflow;
