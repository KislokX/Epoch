//! # Epoch Domain Kernel
//!
//! The innermost layer of the engine: canonical types, **zero I/O**, no knowledge of
//! providers, packs, storage or UI. Everything else depends inward on this crate.
//!
//! The crate boundary is deliberate — "the Kernel depends on nothing" is enforced by
//! the compiler rather than by discipline.
//!
//! See `docs/Architecture/Domain Kernel.md`.
//!
//! ## What lives here today
//!
//! Only what the current milestone step uses (Build From Life, rule 11):
//! the **world concept vocabulary** — place concepts and character archetypes.
//!
//! The engine references concepts. It never references a name, a filename or a
//! franchise (ADR-0016, ADR-0017).

pub mod activity;
mod bridge;
pub mod capability;
pub mod concept;
pub mod context;
pub mod conversation;
pub mod definition;
pub mod knowledge;
pub mod mind;
pub mod place;
pub mod presence;
pub mod quest;
pub mod secret;
pub mod skill;
pub mod trust;

pub use activity::{Activity, ActivityKind, Importance, Subject, Visibility};
pub use bridge::{
    Ask, Declared, Enrol, Enrolled, Have, Hello, HuggingFace, Runner, Shown, Studio, Told, Welcome,
};
pub use capability::{
    Arguments, CapabilityId, Descriptor, Effect, Explanation, Parameter, Reversal, Risk, Value,
    ValueKind,
};
pub use concept::{
    AnchorRole, CharacterArchetype, Direction, MarkRole, PlaceConcept, RendererKind, TerrainKind,
};
pub use context::{compose, Block, Budget, Fate, Outcome, Provenance, Report, Tier};
pub use conversation::{Conversation, Message, Role, ToolCall};
pub use definition::{
    ActionArt, Appearance, CharacterDefinition, CharacterId, IdleBehavior, PresenceProfile,
    Residence, Sheet, Timbre, CHARACTER_SCALE,
};
pub use knowledge::{Approval, KnowledgeKind, KnowledgeObject, Participant};
pub use mind::{
    Brain, CapabilityRequest, ContextPolicy, Control, ControlKind, Mind, Parameters, Reasoning,
    RequestedCapabilities, Tuning, TuningValue,
};
pub use place::PlaceId;
pub use presence::{Action, ActivityClass, Journey, PresenceState};
pub use quest::{
    Artifact, Entry, ImageAttachment, Lifecycle, Quest, QuestId, QuestSessionSettings, QuestState,
    Recorded, Stage, TextAttachment,
};
pub use secret::{Secret, SecretName};
pub use skill::{SkillDefinition, SkillId};
pub use trust::{
    decide, offerable, Autonomy, Because, Decision, Policy, Scope, Situation, Verdict,
};
