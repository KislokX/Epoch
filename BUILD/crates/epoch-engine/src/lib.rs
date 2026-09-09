//! # The Epoch Engine
//!
//! All business logic lives here (ADR-0003). The engine runs **headless** — the desktop
//! shell is one front end over it, never its owner. It speaks canonical kernel types and
//! projects everything that leaves it.
//!
//! ## What lives here today
//!
//! Only what the current milestone step uses (Build From Life, rule 11):
//!
//! - [`pack`] — World Pack loading and concept resolution with the required fallback chain.
//! - [`place`] — Places: the stable identities a World's contributions compose onto.
//! - [`definition`] — the Definition Registry over the vault-file store.
//! - [`simulation`] — the World Simulation: live inhabitants and their presence.
//! - [`world`] — the World projection the shell renders.

pub mod adopt;
pub mod agent;
pub mod agents;
pub mod anthropic;
pub mod asking;
pub mod asset;
pub mod backends;
pub mod bench;
pub mod capabilities;
pub mod capability;
pub mod card;
pub mod context;
pub mod definition;
pub mod deliberation;
pub mod digest;
pub mod disclosure;
pub mod endpoint;
pub mod erasable;
pub mod erase;
pub mod export;
pub mod firstrun;
pub mod optimize;
pub mod orchestrate;
pub mod profiles;
pub mod protocol;
pub mod reference;
pub mod session;
pub mod sessions;
pub mod spec;
pub mod stream;
pub mod suite;
pub mod suite2;
pub mod trials;
pub mod windows;
/// Asking a ComfyUI to draw (ADR-0030).
/// Reaching a ComfyUI.
///
/// **Lives in `epoch-models` because both surfaces need it** — the standing question since
/// 2026-08-21. EpochServices lends computation, and drawing on a lent machine means that machine
/// runs the graph against its own loopback ComfyUI; a second copy of *which picture is the
/// answer* is how the two would eventually disagree about one job. It sits beside `studio`,
/// which was already asking a ComfyUI what it holds.
pub use epoch_models::comfy;
pub mod geography;
pub mod guard;
pub mod handover;
pub mod hiring;
/// Styles, and the workflows that serve them (ADR-0030).
pub mod images;
pub mod import;
pub mod inherits;
pub mod instructions;
pub mod jobs;
pub mod journal;
pub mod kept;
pub mod knowledge;
pub mod launcher;
pub mod library;
pub mod limit;

pub mod map;
pub mod mcp;
/// Authored content Epoch reads, understands and runs.
///
/// Split out of `models` when the difference stopped being cosmetic: that crate answers *what
/// can this machine run*, and a workflow somebody exported from ComfyUI is not a fact about a
/// machine — it is untrusted input from outside that has to be parsed, understood and honestly
/// refused. Re-exported for the same reason as `models`.
pub use epoch_assets as assets;
/// What a machine can run, and what a model costs to run there.
///
/// Its own crate now, because EpochServices asks the same question about the machine lending a
/// graphics card and may not link this one (ADR-0029 §9). Re-exported so every caller here reads
/// exactly as it did — the move is a fact about the build, not about the code.
pub use epoch_models as models;
pub mod bridge;
pub mod hearing;
pub mod openai;
pub mod pack;
pub mod pairing;
pub mod paths;
pub mod pickle;
pub mod place;
pub mod places;
pub mod profile;
pub mod project;
pub mod provider;
pub mod quest;
pub mod ran;
pub mod readiness;
pub mod render;
pub mod rvc;
pub mod secrets;
pub mod serve;
pub mod settings;
pub mod sight;
pub mod simulation;
pub mod skills;
pub mod speech;
pub mod terminal;
pub mod trust;
pub mod turn;
pub mod workshop;
pub mod world;

pub use asset::{AssetError, ResolvedAsset};
pub use capability::{Capability, CapabilityError, CapabilityRegistry, Outcome};
pub use context::user_message;
pub use context::{compose_turn, Ingredients};
pub use definition::{ArtSlot, DefinitionError, DefinitionRegistry};
pub use digest::QuestDigest;
pub use disclosure::{Disclosures, Where};
pub use epoch_models::Machine;
pub use erasable::{erasable, Erasable};
pub use erase::Removal;
pub use import::{accept_image, forget_image, ImageFormat, ImportError};
pub use instructions::Instructions;
pub use journal::{Change, Journal, JournalError};
pub use launcher::{CharacterEdit, CharacterSummary, LauncherView, WorldSummary};
pub use launcher::{LoggedQuest, LoggedRun, ShipsLog};
pub use map::WorldMap;
pub use models::Offer;
pub use pack::{PackError, WorldPack, WorldPackChain};
pub use pairing::{Grant, Paired, Pairings};
pub use place::{
    compose, Anchor, Mark, Place, PlaceContribution, PlaceDeclaration, PlaceId, Placement,
};
pub use profile::Orchestrator;
pub use project::{ProjectError, ProjectRoot, ProjectRoots};
pub use provider::{
    Chunk, KeepLoaded, Ollama, Provider, ProviderError, ProviderRegistry, ProviderStatus, Request,
    Surface,
};
pub use quest::{compose_for, now_ms, QuestError, QuestLog, QuestStore};
pub use render::{Form, ShapeLayer, Tone};
pub use settings::{Settings, SettingsError};
pub use simulation::{CastMember, CharacterInstance, Effort, Simulation};
pub use trust::{TrustEngine, TrustError, TrustStore};
pub use turn::{Completed, Step, Stop, MAX_ROUNDS};
pub use world::{
    AnchorView, CharacterView, FramesView, MapView, MarkView, PlaceView, PlacementView, WorldView,
};
