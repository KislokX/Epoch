//! World concept vocabulary — canonical, versioned, additive-only.
//!
//! The engine knows **concepts**. The active World Pack projects them into an actual
//! world: names, portraits, sprites, labels (ADR-0016, ADR-0017).
//!
//! Adding a variant is additive. Removing one is a breaking change to every pack, so
//! variants are deprecated rather than removed.

use serde::{Deserialize, Serialize};

/// A place in the World, as the engine knows it.
///
/// The engine never knows a location *name* — only which subsystem-bearing place this
/// is. `PlaceConcept::ResearchLab` might be "The Laboratory" in one pack and something
/// entirely different in another; the engine cannot tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaceConcept {
    /// Knowledge Engine — where knowledge is kept.
    KnowledgeCenter,
    /// Models & Integrations — where providers are explored.
    ResearchLab,
    /// Automations — where Quests are orchestrated.
    AutomationHub,
    /// The active Project — where current work is directed.
    CommandCenter,
    /// Party Builder — where Definitions live.
    Guild,
}

impl PlaceConcept {
    /// Every place concept the engine knows. Order is presentation-neutral.
    pub const ALL: [PlaceConcept; 5] = [
        PlaceConcept::KnowledgeCenter,
        PlaceConcept::ResearchLab,
        PlaceConcept::AutomationHub,
        PlaceConcept::CommandCenter,
        PlaceConcept::Guild,
    ];

    /// The canonical, stable identifier used in pack manifests and on the wire.
    pub const fn id(self) -> &'static str {
        match self {
            PlaceConcept::KnowledgeCenter => "knowledge_center",
            PlaceConcept::ResearchLab => "research_lab",
            PlaceConcept::AutomationHub => "automation_hub",
            PlaceConcept::CommandCenter => "command_center",
            PlaceConcept::Guild => "guild",
        }
    }

    /// Parse a canonical id. `None` when no such concept exists — a choice arriving from a
    /// surface is refused here rather than trusted.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.id() == id)
    }
}

/// How a concept is visualized (ADR-0019).
///
/// Vocabulary only. The engine declares that a World *may* say `sprite`; it never learns
/// what a sprite is. The renderer knows how to draw a representation and never knows which
/// concept it is drawing.
///
/// All kinds are declarable from day one so no World manifest becomes invalid later. Phase 1
/// implements only [`RendererKind::Shape`]; a World asking for anything else degrades through
/// the fallback chain to a visible placeholder rather than failing (ADR-0016).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererKind {
    /// Authored vector primitives. The only kind implemented today.
    Shape,
    Sprite,
    AnimatedSprite,
    Tilemap,
    Vector,
    Procedural,
    Model3d,
}

impl RendererKind {
    /// The canonical, stable identifier used in World manifests and on the wire.
    pub const fn id(self) -> &'static str {
        match self {
            RendererKind::Shape => "shape",
            RendererKind::Sprite => "sprite",
            RendererKind::AnimatedSprite => "animated_sprite",
            RendererKind::Tilemap => "tilemap",
            RendererKind::Vector => "vector",
            RendererKind::Procedural => "procedural",
            RendererKind::Model3d => "model3d",
        }
    }

    /// Parse a manifest string, or `None` if no such kind exists.
    ///
    /// Deliberately fallible rather than a `Deserialize` failure: a typo in one Renderable
    /// must cost that Renderable, never the whole World (ADR-0016 — reject the field, not
    /// the World).
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.id() == id)
    }

    pub const ALL: [RendererKind; 7] = [
        RendererKind::Shape,
        RendererKind::Sprite,
        RendererKind::AnimatedSprite,
        RendererKind::Tilemap,
        RendererKind::Vector,
        RendererKind::Procedural,
        RendererKind::Model3d,
    ];

    /// Whether this Epoch build can actually draw this kind.
    ///
    /// Honest capability reporting: a World may declare more than we can render, and the
    /// user should be told rather than shown a blank building.
    ///
    /// `AnimatedSprite` joined the list the day something could play one. Until then it read
    /// `false` and drew a visible placeholder, which is the right answer for an instrument with
    /// nothing behind it — and the wrong one to leave standing once there is.
    pub const fn is_implemented(self) -> bool {
        matches!(
            self,
            RendererKind::Shape | RendererKind::Sprite | RendererKind::AnimatedSprite
        )
    }
}

/// What one **drawn** part of a Place is for (ADR-0021).
///
/// A Place is a composition, not a picture. A Laboratory is a building, plus the shadow that
/// grounds it, plus — eventually — its lights, its particles and the decorations around it.
/// Each of those is a *mark*: something drawn, with a declared purpose.
///
/// The engine never learns what a shadow looks like. It learns that the World called this
/// mark a shadow, so the renderer can treat it as one and the World can override it by name.
///
/// Every role is declarable from day one so no World manifest becomes invalid later. A role
/// this build cannot draw degrades visibly and is reported, exactly like [`RendererKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkRole {
    /// The thing itself: the building, the structure, the object.
    Visual,
    /// The contact shadow that puts it on the ground.
    Shadow,
    Roof,
    Decoration,
    Particles,
    Light,
}

impl MarkRole {
    pub const ALL: [MarkRole; 6] = [
        MarkRole::Visual,
        MarkRole::Shadow,
        MarkRole::Roof,
        MarkRole::Decoration,
        MarkRole::Particles,
        MarkRole::Light,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            MarkRole::Visual => "visual",
            MarkRole::Shadow => "shadow",
            MarkRole::Roof => "roof",
            MarkRole::Decoration => "decoration",
            MarkRole::Particles => "particles",
            MarkRole::Light => "light",
        }
    }

    /// Parse a manifest string, or `None` if no such role exists. Fallible on purpose: a
    /// typo costs one mark, never the whole World.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.id() == id)
    }

    /// Whether this build draws marks of this role, or merely carries them.
    pub const fn is_implemented(self) -> bool {
        matches!(self, MarkRole::Visual | MarkRole::Shadow)
    }
}

/// Which way somebody is facing, as the second half of a canonical asset concept.
///
/// ADR-0016 named `walk.east` on the day it was written; [`crate::Action`] is the first half
/// and this is the second. The engine says *east* and the World Pack decides what east looks
/// like — one row of a sheet, a separate file, or a mirrored west.
///
/// Four, because four is what the World can actually distinguish: a walk is one leg between
/// two Places, and the angle of that leg is the only thing that could ever set this. Eight
/// would be a vocabulary with no measurement behind half of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    North,
    East,
    South,
    West,
}

impl Direction {
    pub const ALL: [Direction; 4] = [
        Direction::North,
        Direction::East,
        Direction::South,
        Direction::West,
    ];

    /// The canonical, stable identifier used in pack manifests and on the wire.
    pub const fn id(self) -> &'static str {
        match self {
            Direction::North => "north",
            Direction::East => "east",
            Direction::South => "south",
            Direction::West => "west",
        }
    }

    /// Parse a manifest string, or `None` if no such direction exists. Fallible for the same
    /// reason every other vocabulary here is: a typo must cost one declaration, never a World.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.id() == id)
    }
}

/// A named point or region a Place declares, for something *other than drawing* to use.
///
/// Anchors are the other half of a composition. Where inhabitants stand, where the name
/// sits, what area responds to a click: all of it is authored by the World rather than
/// computed by the renderer from magic constants.
///
/// Unlike marks, anchors have **no order** — an ordered spawn point means nothing. They are
/// looked up by role, which is why a later contribution declaring the same role replaces it
/// rather than adding a second one: consumers ask "where is the spawn", and that question
/// must have one answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorRole {
    /// Where inhabitants stand when they are here.
    Spawn,
    /// Where the Place's name sits.
    Label,
    /// The region that responds to hover, focus and click.
    InteractionBounds,
    Entrance,
    CameraFocus,
    InteriorLink,
}

impl AnchorRole {
    pub const ALL: [AnchorRole; 6] = [
        AnchorRole::Spawn,
        AnchorRole::Label,
        AnchorRole::InteractionBounds,
        AnchorRole::Entrance,
        AnchorRole::CameraFocus,
        AnchorRole::InteriorLink,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            AnchorRole::Spawn => "spawn",
            AnchorRole::Label => "label",
            AnchorRole::InteractionBounds => "interaction_bounds",
            AnchorRole::Entrance => "entrance",
            AnchorRole::CameraFocus => "camera_focus",
            AnchorRole::InteriorLink => "interior_link",
        }
    }

    /// Parse a manifest string, or `None` if no such role exists. Fallible on purpose: a
    /// typo costs one anchor, never the whole World.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.id() == id)
    }

    /// Whether anything in this build consumes anchors of this role.
    pub const fn is_implemented(self) -> bool {
        matches!(
            self,
            AnchorRole::Spawn | AnchorRole::Label | AnchorRole::InteractionBounds
        )
    }
}

/// A kind of terrain, as the engine knows it.
///
/// Vocabulary only. The engine never knows *where* terrain is — geography is presentation
/// and belongs to the active World Pack (ADR-0016). This exists so a pack can say "forest"
/// in terms the engine recognises, exactly as it says "research_lab".
///
/// Geography is part of Epoch's language: a forest communicates the less-travelled, a
/// mountain communicates difficulty, water communicates separation (Build From Life 18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerrainKind {
    Grass,
    Forest,
    Water,
    Mountain,
    Stone,
    Sand,
}

impl TerrainKind {
    pub const ALL: [TerrainKind; 6] = [
        TerrainKind::Grass,
        TerrainKind::Forest,
        TerrainKind::Water,
        TerrainKind::Mountain,
        TerrainKind::Stone,
        TerrainKind::Sand,
    ];

    /// The canonical, stable identifier used in pack manifests and on the wire.
    pub const fn id(self) -> &'static str {
        match self {
            TerrainKind::Grass => "grass",
            TerrainKind::Forest => "forest",
            TerrainKind::Water => "water",
            TerrainKind::Mountain => "mountain",
            TerrainKind::Stone => "stone",
            TerrainKind::Sand => "sand",
        }
    }
}

/// A character archetype, as the engine knows it.
///
/// The engine never knows a character *name*. Behaviour belongs to the archetype and is
/// constant across every pack; the name and face are supplied by the active World Pack
/// (ADR-0017, `CHARACTER_BIBLE.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterArchetype {
    Researcher,
    Coordinator,
    Guardian,
    Historian,
}

impl CharacterArchetype {
    pub const ALL: [CharacterArchetype; 4] = [
        CharacterArchetype::Researcher,
        CharacterArchetype::Coordinator,
        CharacterArchetype::Guardian,
        CharacterArchetype::Historian,
    ];

    /// The canonical, stable identifier used in definitions, pack manifests and on the wire.
    pub const fn id(self) -> &'static str {
        match self {
            CharacterArchetype::Researcher => "researcher",
            CharacterArchetype::Coordinator => "coordinator",
            CharacterArchetype::Guardian => "guardian",
            CharacterArchetype::Historian => "historian",
        }
    }

    /// Parse a canonical id. `None` when no such archetype exists.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.id() == id)
    }

    /// The place this archetype calls home, as a concept.
    ///
    /// Authored behaviour, not presentation: it is the same in every pack.
    pub const fn home_place(self) -> PlaceConcept {
        match self {
            CharacterArchetype::Researcher => PlaceConcept::ResearchLab,
            CharacterArchetype::Coordinator => PlaceConcept::CommandCenter,
            CharacterArchetype::Guardian => PlaceConcept::KnowledgeCenter,
            CharacterArchetype::Historian => PlaceConcept::KnowledgeCenter,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn place_ids_are_unique_and_snake_case() {
        let mut seen = Vec::new();
        for p in PlaceConcept::ALL {
            let id = p.id();
            assert!(!seen.contains(&id), "duplicate place id: {id}");
            assert!(
                id.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "place id not snake_case: {id}"
            );
            seen.push(id);
        }
    }

    #[test]
    fn archetype_ids_are_unique() {
        let mut seen = Vec::new();
        for a in CharacterArchetype::ALL {
            let id = a.id();
            assert!(!seen.contains(&id), "duplicate archetype id: {id}");
            seen.push(id);
        }
    }

    #[test]
    fn every_composition_role_is_declarable_and_uniquely_identified() {
        // A World must be able to declare a role we do not draw yet without its manifest
        // becoming invalid later (ADR-0021). Ids are the stable authoring vocabulary, so a
        // collision would silently merge two different intentions.
        let mut seen = Vec::new();
        for r in MarkRole::ALL {
            assert!(
                !seen.contains(&r.id()),
                "duplicate mark role id: {}",
                r.id()
            );
            seen.push(r.id());
        }
        let mut seen = Vec::new();
        for r in AnchorRole::ALL {
            assert!(
                !seen.contains(&r.id()),
                "duplicate anchor role id: {}",
                r.id()
            );
            seen.push(r.id());
        }

        // And some of each must actually be drawable, or a Place could never render.
        assert!(MarkRole::ALL.iter().any(|r| r.is_implemented()));
        assert!(AnchorRole::ALL.iter().any(|r| r.is_implemented()));
    }

    #[test]
    fn every_archetype_has_a_home_place() {
        // A character always has a current place (ADR-0018). Home is where they start.
        for a in CharacterArchetype::ALL {
            assert!(PlaceConcept::ALL.contains(&a.home_place()));
        }
    }
}
