//! Where the Places are, for the one subsystem that has to walk between them.
//!
//! ## Why this exists rather than the Simulation reading the map
//!
//! The Simulation needs two facts about a Place — where it is, and what it is called — and
//! nothing else. Handing it the whole composed World would give it a `WorldPackChain`, artwork,
//! draw order and anchors, all of which it would have to be trusted not to use. This is the
//! narrow view instead: a lookup, built once from the same composition the renderer draws from,
//! so the distance somebody walks is the distance the user can see.
//!
//! ## Unplaced is a real answer
//!
//! A Place with no position exists (`Place::placement` has always been an `Option` — a partly
//! authored World is allowed). It simply is not in here. Everything downstream reads that as
//! *there is nowhere to walk to*, and nobody walks — rather than walking to the origin, which
//! is the shape of lie where a character ends up standing in the sea.

use std::collections::BTreeMap;

use epoch_kernel::{Direction, PlaceId};

/// A Place, as far as travel is concerned.
#[derive(Debug, Clone, PartialEq)]
struct Landmark {
    x: f32,
    y: f32,
    /// What it is called, so a character can say where they are going in the user's own words.
    ///
    /// Carried rather than looked up later: the Simulation composes the sentence, and a
    /// surface that had to join a `PlaceId` back to a title would be re-deriving something the
    /// Engine already had.
    title: String,
}

/// Every positioned Place in the World.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Geography {
    landmarks: BTreeMap<PlaceId, Landmark>,
    /// The authored road between two Places, by the pair it joins.
    ///
    /// **Here because a bearing is not a facing.** A leg's direction is the straight line
    /// between two Places; what somebody actually walks is a road with corners. Measured on
    /// this World: the Command Center is 320 units east of the Tower and 126 north of it, so
    /// the leg bears *east* — while the stretch of road in front of the Tower runs straight
    /// down. A character walked south down that stretch drawn facing east, which is the World
    /// saying something untrue about itself.
    ///
    /// Empty is the ordinary case for a World with no `places.toml`, and then the straight line
    /// is the best answer anybody has.
    roads: BTreeMap<(PlaceId, PlaceId), Vec<[f32; 2]>>,
}

impl Geography {
    /// Read the positions out of composed Places.
    ///
    /// Takes what it needs rather than the type it came from, so the caller decides how the
    /// World was composed and this stays a lookup.
    pub fn of<'a>(places: impl IntoIterator<Item = &'a crate::place::Place>) -> Self {
        Self {
            landmarks: places
                .into_iter()
                .filter_map(|place| {
                    let at = place.placement?;
                    Some((
                        place.id.clone(),
                        Landmark {
                            x: at.x,
                            y: at.y,
                            title: place.title.clone(),
                        },
                    ))
                })
                .collect(),
            // Added by `with_roads` when the World has a `places.toml`. Without one every
            // journey is a straight line, and the straight-line bearing is then exactly true.
            roads: BTreeMap::new(),
        }
    }

    /// What a Place is called, if this World has it.
    pub fn title(&self, place: &PlaceId) -> Option<&str> {
        self.landmarks.get(place).map(|l| l.title.as_str())
    }

    /// Whether this World has a positioned Place by that name.
    pub fn has(&self, place: &PlaceId) -> bool {
        self.landmarks.contains_key(place)
    }

    /// How far apart two Places are, in world units.
    ///
    /// `None` when either end is missing — the same answer as "there is nowhere to walk to",
    /// and the caller may not distinguish them, because it should behave identically for both.
    ///
    /// Straight-line, deliberately. Roads exist and a road may bend, but a distance measured
    /// along a route the World only sometimes has would make travel time depend on whether
    /// somebody happened to draw a road — so two identical Worlds, one with roads and one
    /// without, would disagree about how long a walk takes.
    pub fn distance(&self, from: &PlaceId, to: &PlaceId) -> Option<f32> {
        let a = self.landmarks.get(from)?;
        let b = self.landmarks.get(to)?;
        Some(((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt())
    }

    /// Which way somebody walking from one Place to the other is facing.
    ///
    /// **Measured here rather than in a renderer.** A direction is a fact about the World's
    /// geography, and a UI that computed it would be deciding something about the world from
    /// coordinates it happens to be drawing with — the same shape of mistake as a client
    /// inventing a position (ADR-0018).
    ///
    /// Four, because four is what a sprite sheet distinguishes: whichever axis the walk covers
    /// more of wins, and a perfect diagonal falls to the horizontal because a side-on walk
    /// reads better than a back or a front at the same angle.
    ///
    /// `None` when either end is missing — the same answer as "there is nowhere to walk to".
    /// Remember the roads, so a facing can follow one.
    ///
    /// Keyed both ways round: a road is walked in either direction and the caller should not
    /// have to know which way it was authored.
    pub fn with_roads<'a>(
        mut self,
        roads: impl IntoIterator<Item = (&'a PlaceId, &'a PlaceId, &'a [[f32; 2]])>,
    ) -> Self {
        for (from, to, via) in roads {
            let (Some(a), Some(b)) = (self.landmarks.get(from), self.landmarks.get(to)) else {
                // A road to a Place this World does not compose is not a road anybody walks.
                continue;
            };
            let mut points = Vec::with_capacity(via.len() + 2);
            points.push([a.x, a.y]);
            points.extend_from_slice(via);
            points.push([b.x, b.y]);
            let back: Vec<[f32; 2]> = points.iter().copied().rev().collect();
            self.roads.insert((from.clone(), to.clone()), points);
            self.roads.insert((to.clone(), from.clone()), back);
        }
        self
    }

    /// Which way somebody is facing **at this point of the walk**.
    ///
    /// The direction of the stretch of road under their feet, rather than of the whole journey.
    /// `None` when no road joins the two Places — and then [`Geography::bearing`] is the honest
    /// answer, because a World with no authored road really is a straight line.
    pub fn facing_along(&self, from: &PlaceId, to: &PlaceId, progress: f32) -> Option<Direction> {
        let points = self.roads.get(&(from.clone(), to.clone()))?;
        if points.len() < 2 {
            return None;
        }

        // The same walk the renderer does along the same polyline, so the direction belongs to
        // the segment it is actually drawing somebody on.
        let lengths: Vec<f32> = points
            .windows(2)
            .map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]))
            .collect();
        let total: f32 = lengths.iter().sum();
        if total <= 0.0 {
            return None;
        }

        let mut left = progress.clamp(0.0, 1.0) * total;
        for (index, length) in lengths.iter().enumerate() {
            if left > *length && index + 2 < points.len() {
                left -= length;
                continue;
            }
            let a = points[index];
            let b = points[index + 1];
            return Some(Self::heading(b[0] - a[0], b[1] - a[1]));
        }
        None
    }

    /// Which of the four a movement is, closest wins.
    ///
    /// One function, because the straight-line bearing and the along-the-road one must agree
    /// about where east stops and south begins — two copies would eventually disagree at the
    /// diagonal, and a character would change direction by being looked at differently.
    fn heading(dx: f32, dy: f32) -> Direction {
        if dx.abs() >= dy.abs() {
            if dx >= 0.0 {
                Direction::East
            } else {
                Direction::West
            }
        } else if dy >= 0.0 {
            // Screen coordinates: y grows downward, so more y is towards the viewer.
            Direction::South
        } else {
            Direction::North
        }
    }

    pub fn bearing(&self, from: &PlaceId, to: &PlaceId) -> Option<Direction> {
        let a = self.landmarks.get(from)?;
        let b = self.landmarks.get(to)?;
        Some(Self::heading(b.x - a.x, b.y - a.y))
    }

    pub fn is_empty(&self) -> bool {
        self.landmarks.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The defect, with the World's own numbers.
    #[test]
    fn a_facing_follows_the_road_rather_than_the_journey() {
        // Measured from `vault/worlds/default/places.toml`: the Command Center is 320 units
        // east of the Tower and 126 north of it, so the straight line between them bears
        // **east** — while the stretch of road in front of the Tower runs straight down. Mage
        // walked south down it drawn facing east for the whole leg.
        let world = Geography::of(&[
            place("command_center", Some((494.0, 1052.0))),
            place("research_lab", Some((814.0, 926.0))),
        ])
        .with_roads([(
            &PlaceId::new("command_center"),
            &PlaceId::new("research_lab"),
            // Out east first, then straight down to the Tower.
            [[814.0f32, 1052.0]].as_slice(),
        )]);

        let from = PlaceId::new("command_center");
        let to = PlaceId::new("research_lab");

        // The journey as a whole still bears east, and that reading is not wrong — it is
        // answering a different question.
        assert_eq!(world.bearing(&from, &to), Some(Direction::East));

        // But the first stretch runs east and the last runs north (up the screen, towards the
        // Tower), and that is what somebody walking it is doing.
        assert_eq!(world.facing_along(&from, &to, 0.1), Some(Direction::East));
        assert_eq!(world.facing_along(&from, &to, 0.95), Some(Direction::North));

        // And walked the other way, the same road turns the other way.
        assert_eq!(world.facing_along(&to, &from, 0.1), Some(Direction::South));
        assert_eq!(world.facing_along(&to, &from, 0.95), Some(Direction::West));
    }

    #[test]
    fn a_world_with_no_road_between_two_places_says_so() {
        // Then the straight line is exactly true, and the caller falls back to it rather than
        // being handed a direction nobody measured.
        let world = Geography::of(&[
            place("a", Some((0.0, 0.0))),
            place("b", Some((100.0, 10.0))),
        ]);
        assert_eq!(
            world.facing_along(&PlaceId::new("a"), &PlaceId::new("b"), 0.5),
            None
        );
        assert_eq!(
            world.bearing(&PlaceId::new("a"), &PlaceId::new("b")),
            Some(Direction::East)
        );
    }

    use crate::place::{Place, Placement};

    fn place(id: &str, at: Option<(f32, f32)>) -> Place {
        Place {
            id: PlaceId::new(id),
            concept: None,
            title: format!("The {id}"),
            subtitle: None,
            is_placeholder: false,
            placement: at.map(|(x, y)| Placement {
                x,
                y,
                footprint: 90.0,
                z_order: 0,
            }),
            marks: Vec::new(),
            anchors: Vec::new(),
        }
    }

    #[test]
    fn distance_is_the_one_the_user_can_see() {
        let world = Geography::of(&[
            place("a", Some((0.0, 0.0))),
            place("b", Some((300.0, 400.0))),
        ]);
        // 3-4-5, in the same units the map is drawn in.
        assert_eq!(
            world.distance(&PlaceId::new("a"), &PlaceId::new("b")),
            Some(500.0)
        );
    }

    #[test]
    fn an_unplaced_place_is_nowhere_to_walk_to() {
        // Not an error and not the origin. A World can be half-built; nobody sets out for a
        // building that has not been put anywhere.
        let world = Geography::of(&[place("a", Some((0.0, 0.0))), place("nowhere", None)]);

        assert!(!world.has(&PlaceId::new("nowhere")));
        assert_eq!(
            world.distance(&PlaceId::new("a"), &PlaceId::new("nowhere")),
            None
        );
        // And a Place this World has never heard of behaves exactly the same way.
        assert_eq!(
            world.distance(&PlaceId::new("a"), &PlaceId::new("atlantis")),
            None
        );
    }

    #[test]
    fn a_place_carries_the_name_the_user_gave_it() {
        // So somebody can say where they are going without a second lookup joining an id back
        // to a title — and without the Engine inventing a name for a building.
        let world = Geography::of(&[place("library", Some((10.0, 10.0)))]);
        assert_eq!(world.title(&PlaceId::new("library")), Some("The library"));
        assert_eq!(world.title(&PlaceId::new("atlantis")), None);
    }

    #[test]
    fn a_walk_faces_whichever_axis_it_covers_more_of() {
        // Four directions because four is what a sheet distinguishes, and the axis that wins
        // is the one with more ground to cover — a walk that is mostly sideways reads sideways.
        let world = Geography::of(&[
            place("home", Some((0.0, 0.0))),
            place("east", Some((100.0, 10.0))),
            place("west", Some((-100.0, 10.0))),
            place("south", Some((10.0, 100.0))),
            place("north", Some((10.0, -100.0))),
        ]);
        let home = PlaceId::new("home");
        for (place, expected) in [
            ("east", Direction::East),
            ("west", Direction::West),
            ("south", Direction::South),
            ("north", Direction::North),
        ] {
            assert_eq!(world.bearing(&home, &PlaceId::new(place)), Some(expected));
        }
    }

    #[test]
    fn a_perfect_diagonal_falls_to_the_horizontal() {
        // Deliberate rather than incidental: a side-on walk reads better than a back or a
        // front at the same angle, and something has to win.
        let world = Geography::of(&[
            place("home", Some((0.0, 0.0))),
            place("corner", Some((50.0, 50.0))),
        ]);
        assert_eq!(
            world.bearing(&PlaceId::new("home"), &PlaceId::new("corner")),
            Some(Direction::East)
        );
    }

    #[test]
    fn a_place_nobody_positioned_has_no_bearing() {
        let world = Geography::of(&[place("home", Some((0.0, 0.0)))]);
        assert_eq!(
            world.bearing(&PlaceId::new("home"), &PlaceId::new("nowhere")),
            None
        );
    }
}
