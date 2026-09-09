//! World geography — **pack data, carried but never interpreted**.
//!
//! The engine knows only *which place* a character is in (ADR-0018). It does not know where
//! places are, how far apart they sit, or what lies between them. That is geography, and
//! geography is presentation: it belongs to the active World Pack (ADR-0016).
//!
//! This module parses a pack's map and hands it to the projection. The engine performs no
//! geometry: it never measures a distance, never tests containment, never routes a path.
//! Swapping the pack swaps the world's geography, and the engine cannot tell.
//!
//! ## World units, not pixels
//!
//! Everything here is in **world units**. Pixels belong to the viewport; world units belong
//! to the place. A world is deliberately authored larger than it is currently occupied —
//! empty land is where cities go (Build From Life 18, "think world, not screen").

use epoch_kernel::TerrainKind;
use serde::{Deserialize, Serialize};

/// A point in world units.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// The extent of the world, in world units.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct WorldSize {
    pub width: f32,
    pub height: f32,
}

/// One area of terrain: a kind, and the polygon it covers.
///
/// Polygons rather than a tile grid: a handful of points is authorable by hand and reads as
/// an organic coastline or forest edge. A future pack may express terrain differently —
/// the engine carries whatever the pack declares (Earn Complexity).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct TerrainArea {
    pub kind: TerrainKind,
    /// Polygon outline, in world units. Rendered in declaration order, so later areas sit
    /// on top of earlier ones.
    pub points: Vec<Point>,
}

/// How travelled a route is, as the world's own history — not a claim about this session.
///
/// Authored world-building: it says "in this world, this is a major road". It never asserts
/// that anyone walked it today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Prominence {
    Major,
    Minor,
}

/// A route between two Places, named by **identity** rather than by concept.
///
/// Referencing Places by id is what lets a road survive everything about a Place changing
/// except what it is: its appearance, its name, its assets and its size may all be replaced
/// by another layer, and the road still arrives (ADR-0022).
///
/// **A pack declares that two Places are joined. It does not declare where the road bends.**
/// It used to: `waypoints` carried intermediate corners so a road could go around water. Then
/// ADR-0028 gave *positions* to the user, and a corner is only meaningful relative to the two
/// ends it bends between - so the bends were left anchored to nothing. Measured: the default
/// pack's corners were authored against a 3600x2200 map, the user painted 1536x1024 land and
/// moved the buildings, and its roads ran out into open ocean and back.
///
/// A corner belongs to whoever placed the buildings, and that is the user; they trace one in
/// World Editor and it is theirs (`places::Road::via`). What a pack can still honestly say is
/// *that* there is a road, and how travelled it is.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Route {
    pub from: String,
    pub to: String,
    #[serde(default = "default_prominence")]
    pub prominence: Prominence,
}

fn default_prominence() -> Prominence {
    Prominence::Minor
}

/// A pack's geography: the land, and the roads across it.
///
/// Where Places *sit* is no longer here. Position belongs to the Place, alongside its
/// identity and its appearance, so that authoring a Place is one contiguous block rather
/// than three sections of a manifest to keep in sync (ADR-0022).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct WorldMap {
    pub size: WorldSize,
    #[serde(default)]
    pub terrain: Vec<TerrainArea>,
    #[serde(default)]
    pub routes: Vec<Route>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP: &str = r#"
[size]
width = 3200.0
height = 1800.0

[[terrain]]
kind = "grass"
points = [ { x = 0.0, y = 0.0 }, { x = 3200.0, y = 0.0 }, { x = 3200.0, y = 1800.0 }, { x = 0.0, y = 1800.0 } ]

[[routes]]
from = "command_center"
to = "guild"
prominence = "major"

[[routes]]
from = "command_center"
to = "research_lab"
waypoints = [ { x = 1200.0, y = 950.0 } ]
"#;

    #[test]
    fn a_pack_no_longer_bends_a_road_it_did_not_place() {
        // The corners of a pack authored before ADR-0028 are read and dropped rather than
        // refused: they were valid when they were written, and a World that will not load is a
        // worse answer than a World whose roads run straight between real doors.
        let map: WorldMap = toml::from_str(MAP).unwrap();
        assert_eq!(map.routes.len(), 2);
    }

    #[test]
    fn a_map_parses_from_a_pack_manifest() {
        let map: WorldMap = toml::from_str(MAP).unwrap();
        assert_eq!(map.size.width, 3200.0);
        assert_eq!(map.terrain.len(), 1);
        assert_eq!(map.terrain[0].kind, TerrainKind::Grass);
        assert_eq!(map.routes[0].prominence, Prominence::Major);
    }

    #[test]
    fn a_road_names_the_places_it_joins_by_identity() {
        // Not by concept: a World may reshape what a Place looks like and what it is called,
        // and the road must still arrive.
        let map: WorldMap = toml::from_str(MAP).unwrap();
        assert_eq!(map.routes[1].from, "command_center");
        assert_eq!(map.routes[1].to, "research_lab");
    }

    #[test]
    fn a_road_is_minor_unless_the_world_says_otherwise() {
        let map: WorldMap = toml::from_str(MAP).unwrap();
        assert_eq!(map.routes[1].prominence, Prominence::Minor);
    }
}
