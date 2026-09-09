//! The World view — what the shell renders.
//!
//! This is a **projection** (Internal First, ADR-0003). The engine reasons in concepts and
//! identities; what leaves it carries the names the active World supplied, so the UI never
//! needs to know a concept id, and the engine never needs to know a name.
//!
//! Rendering is only one projection of a Place. Nothing here is privileged: a headless
//! caller composes the same Places and asks them the same questions.

use std::collections::BTreeMap;

use epoch_kernel::{Action, ActivityClass, PlaceConcept};

/// The closed action vocabulary, as one word on the wire.
///
/// A function rather than `Serialize` on the enum, because what crosses this boundary is a
/// projection and the Kernel's spelling is not a promise the UI gets to depend on (Internal
/// First). `walk` and not `walking`: the renderer asks a pack for `walk.east`.
fn action_name(action: Action) -> &'static str {
    match action {
        Action::Idle => "idle",
        Action::Walk => "walk",
        Action::Settle => "settle",
        Action::Talk => "talk",
        Action::Think => "think",
        Action::Work => "work",
    }
}
use serde::Serialize;

use crate::map::Prominence;
use crate::pack::WorldPackChain;
use crate::place::{self, Place, Placement};
use crate::simulation::CastMember;

/// One Place, ready to draw.
///
/// Serialized in camelCase: field naming at the boundary is part of the projection, so the
/// presentation layer reads idiomatic TypeScript without the engine bending to it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceView {
    /// Stable identity. Everything references a Place by this, never by appearance.
    pub id: String,
    /// Canonical concept id — what capability this Place is the visible projection of.
    /// What kind of Place this is, when it is a kind the Engine can animate. `null` for a
    /// Place the user invented — it exists and it simply lights up for nobody (ADR-0028).
    pub concept: Option<&'static str>,
    /// What this Place is called in the active World.
    pub title: String,
    /// What it is for, in the World's own words. Shown when a Place is selected.
    pub subtitle: Option<String>,
    /// True when no contributor named it and the title is a visible stand-in.
    pub is_placeholder: bool,
    /// Where it sits. `None` means known but not positioned — the World still has it.
    pub placement: Option<PlacementView>,
    /// Everything drawn, in draw order.
    pub marks: Vec<MarkView>,
    /// Named points and regions, for things other than drawing.
    pub anchors: Vec<AnchorView>,
}

/// A Place's position, size and ordering, in world units.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacementView {
    pub x: f32,
    pub y: f32,
    /// Footprint in world units. This is the Place's scale; Places are not uniform, and
    /// scale is information.
    pub footprint: f32,
    /// Explicit draw order. Places are already emitted in order; this is carried so a
    /// renderer with its own scene graph can reproduce it.
    pub z_order: i32,
}

/// One drawn part of a Place: a purpose, and how to draw it.
///
/// The engine carries this unread. It does not know what a `shape` is, only that the World
/// said so and handed over layers.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkView {
    /// Canonical mark role id (e.g. "visual", "shadow").
    pub role: &'static str,
    /// Canonical renderer kind id (e.g. "shape", "sprite").
    pub renderer: &'static str,
    /// True when this build can draw both the role and the renderer. When false the
    /// renderer must show a placeholder and say why — never a blank space.
    pub supported: bool,
    /// A `data:` URI for the World's Asset, when it declared one that could be read.
    ///
    /// The renderer draws this with `<image>` and never inlines it as markup: a World is
    /// third-party content (ADR-0020).
    pub asset: Option<String>,
    /// Size relative to the Place footprint.
    pub scale: f32,
    /// Asset origin in its own normalised space; `[0.5, 1.0]` is bottom-centre.
    pub anchor: [f32; 2],
    /// Layers for the `shape` renderer, in draw order. Geometry is in footprint units.
    pub shape: Vec<ShapeLayerView>,
    /// How to cut the asset, when it is a sheet rather than a picture.
    ///
    /// Absent for every still mark, which is nearly all of them — so a World with no animation
    /// sends exactly what it sent before this existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frames: Option<FramesView>,
}

/// How a sheet is cut, as the presentation layer receives it.
///
/// The cut, never the playback. Which cell is on screen at this millisecond is a per-frame
/// question and it belongs to the renderer — the same line ADR-0018 drew for characters.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FramesView {
    pub columns: u32,
    pub rows: u32,
    /// Real cells, in reading order from the top left. Always a number; never implied.
    pub count: u32,
    /// How long one cell is shown.
    pub milliseconds: u32,
    /// Canonical direction ids (`"north"`, `"east"`, …), one per **row**, in row order.
    ///
    /// Empty means the sheet is not directional: one loop, whichever way somebody is facing.
    pub directions: Vec<&'static str>,
}

/// A named point or region a Place declares, in footprint units.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnchorView {
    /// Canonical anchor role id (e.g. "spawn", "label", "interaction_bounds").
    pub role: &'static str,
    /// True when something in this build actually consumes this role.
    pub supported: bool,
    pub x: f32,
    pub y: f32,
    /// Half-width. `0` means this anchor is a point rather than a region.
    pub w: f32,
    /// Half-height. `0` means a point.
    pub h: f32,
}

/// One drawn layer of a `shape` Renderable.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapeLayerView {
    /// `"rect"`, `"polygon"`, `"dome"` or `"ellipse"`.
    pub form: &'static str,
    /// Colour role: `"wall"`, `"roof"`, `"detail"`, `"accent"` or `"shadow"`.
    pub tone: &'static str,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub r: f32,
    pub points: Vec<[f32; 2]>,
}

/// The world's geography, as the World authored it.
///
/// The engine carries this without interpreting it: it performs no geometry, measures no
/// distance and routes no path. Geography is presentation (ADR-0016).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapView {
    /// Extent in world units. Deliberately larger than the occupied area — empty land is
    /// where future cities go.
    pub width: f32,
    pub height: f32,
    pub terrain: Vec<TerrainView>,
    pub routes: Vec<RouteView>,
}

/// One area of terrain, in draw order.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerrainView {
    /// Canonical terrain kind id (e.g. "forest").
    pub kind: &'static str,
    /// Polygon outline as flat `[x, y]` pairs, in world units.
    pub points: Vec<[f32; 2]>,
}

/// A route between two Places, already resolved to world-unit points.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteView {
    /// Full polyline from origin Place to destination Place, including waypoints.
    pub points: Vec<[f32; 2]>,
    /// `"major"` or `"minor"` — how travelled this route is in the world's own history.
    pub prominence: &'static str,
}

/// One inhabitant, ready to draw.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterView {
    /// **Who** this is. Stable identity, safe to key on and safe to send back in a command.
    pub id: String,
    /// What they are called. Theirs, not the World's (ADR-0023).
    pub name: String,
    /// Canonical archetype id (e.g. "researcher") — what kind of worker they are. Several
    /// characters may share one, so this classifies and never identifies.
    pub archetype: &'static str,
    /// **Which Place is theirs** in this World — where they return to, not where they are.
    ///
    /// Separate from `place` on purpose. The editor assigns a home; the World shows a location;
    /// somebody who is out is genuinely elsewhere and both facts stay true (ADR-0018).
    pub home: String,
    /// **Which Place** they are currently in — an identity, not a concept (ADR-0028).
    ///
    /// `String` rather than `&'static str` because a Place is the user's now: its id comes from
    /// a vault file, not from an enum this build compiled in. A borrowed `'static` here was
    /// itself a quiet statement that the set of places was closed.
    pub place: String,
    /// What they are visibly doing, in their world's terms.
    pub activity: String,
    /// Which action to draw — `"idle"`, `"walk"`, `"think"` or `"work"`.
    ///
    /// Beside `class`, never instead of it. `class` is the honesty rule (routine may not look
    /// like work); this is the animation. A renderer that read one for the other would decide
    /// a question Build From Life rule 3 does not leave to renderers.
    pub action: &'static str,
    /// `"idle"` or `"work"`. The World must render these differently: routine behaviour
    /// may never look like work that is not happening.
    pub class: &'static str,
    /// How they look. `None` means nobody gave them a face, and the renderer shows a visible
    /// stand-in rather than inventing one.
    ///
    /// The same `MarkView` a Place uses: a character's appearance is not a second pipeline.
    pub mark: Option<MarkView>,
    /// The face that identifies them — used where a portrait is what matters rather than a
    /// body: a conversation, a roster row. Falls back to the sprite when they have no icon.
    pub icon: Option<MarkView>,
    /// What they look like doing each thing they have been drawn doing, keyed by action id.
    ///
    /// Beside `mark` rather than instead of it: `mark` is the still picture, and it is what a
    /// renderer falls back to for every action **not** in here — which is every action for a
    /// character with one drawing, and that character is complete (ADR-0016's chain).
    ///
    /// Empty rather than absent, because a surface asks "is there one for `walk`?" and an
    /// object that is sometimes missing turns one question into two.
    pub actions: BTreeMap<&'static str, MarkView>,
    /// Where they are going, when they are going somewhere.
    ///
    /// Absent for the overwhelmingly common case of somebody standing still, which is also what
    /// keeps this projection the same size it was for every World that has no travel in it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journey: Option<JourneyView>,
    /// What they do when nothing is asked, and where each step happens in this World.
    ///
    /// Here because the World Editor is where somebody decides the second half, and it cannot
    /// offer a choice about a routine it cannot see. Small - a few words per step - which is the
    /// bar for anything that rides the projection at all.
    pub routine: Vec<RoutineStepView>,
    /// Which voice they speak with — the name, exactly as authored. `None` is silence.
    ///
    /// Here because the World is where speaking happens: the Chronicle is the only place an
    /// answer arrives, and asking the Launcher for a character's file every time somebody
    /// finishes a sentence would be a second reader of the same fact.
    ///
    /// **Never checked against what is installed.** That is a fact about the machine and it
    /// changes when somebody installs a voice; whether a character chose one is a fact about
    /// them, and the two must stay distinguishable.
    pub speaks_with: Option<String>,
    /// Which converted RVC voice colours it, and at what pitch. `None` is the plain voice.
    ///
    /// Beside `speaks_with` for the same reason it is here at all, and **as one field**: the
    /// pitch is a property of the pair, so a projection that split them would let a surface
    /// hold a pitch for a model nobody chose.
    pub sounds_like: Option<epoch_kernel::Timbre>,
}

/// One step of a routine, and where it happens.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutineStepView {
    pub activity: String,
    pub seconds: u32,
    /// The Place this happens at **in this World**, or `None` for at home (ADR-0028).
    pub place: Option<String>,
}

/// A walk, as the presentation layer receives it.
///
/// The four numbers ADR-0018 said the UI needs to interpolate honestly. It draws sixty frames a
/// second between messages that arrive every few seconds; with a speed and an ETA it is deriving
/// the answer the Engine would give rather than inventing one, and the next authoritative state
/// always wins.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JourneyView {
    pub from: String,
    pub to: String,
    pub progress: f32,
    pub speed: f32,
    pub eta_seconds: f32,
    /// Which way they are walking — `"north"`, `"east"`, `"south"` or `"west"`.
    ///
    /// Measured from the World's geography, so a renderer picking the row of a directional
    /// sheet is reading an answer rather than working one out.
    pub facing: &'static str,
}

impl JourneyView {
    fn of(journey: &epoch_kernel::Journey) -> Self {
        Self {
            from: journey.from.to_string(),
            to: journey.to.to_string(),
            progress: journey.progress,
            speed: journey.speed,
            eta_seconds: journey.eta_seconds,
            facing: journey.facing.id(),
        }
    }
}

/// Somebody's presence on its own, without the World around it.
///
/// **Why this exists at all:** `world:changed` carries every Place, every mark and every face,
/// and it was the only way the World heard about anybody. That was fine while presence changed
/// every few minutes; a character walking across the map changes it continuously, and re-sending
/// the whole World ten times a walk is the same mistake the backdrop already taught us
/// (`CLAUDE.md`: large assets stay out of the per-tick projection).
///
/// So movement rides its own channel and carries only what moved. The full projection is still
/// what arrives when the World's *shape* changes — somebody set out, somebody arrived, somebody
/// started work — because those are the moments a surface may need more than a position.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresenceView {
    pub id: String,
    pub place: String,
    pub activity: String,
    pub action: &'static str,
    pub class: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journey: Option<JourneyView>,
}

impl PresenceView {
    pub fn of(presence: &epoch_kernel::PresenceState) -> Self {
        Self {
            id: presence.character.to_string(),
            place: presence.place.to_string(),
            activity: presence.activity.clone(),
            action: action_name(presence.action),
            class: match presence.class {
                ActivityClass::Idle => "idle",
                ActivityClass::Work => "work",
                ActivityClass::Waiting => "waiting",
            },
            journey: presence.journey.as_ref().map(JourneyView::of),
        }
    }
}

/// The World as the shell receives it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldView {
    /// The active World's display name, or `None` when none is loaded.
    pub pack_name: Option<String>,
    /// The world's geography, if the active chain declares one.
    pub map: Option<MapView>,
    pub places: Vec<PlaceView>,
    pub characters: Vec<CharacterView>,
}

impl WorldView {
    /// Project the World: every Place, and everyone currently in it.
    ///
    /// Always succeeds. The chain terminates in a placeholder and an empty world is a valid
    /// world, so there is never a reason to show a loading screen instead of the World
    /// (Build From Life, rule 1).
    pub fn project(chain: &WorldPackChain, cast: &[CastMember]) -> Self {
        Self::project_with(chain, cast, &crate::places::WorldMap::default())
    }

    /// Project the World with the user's own map laid over the pack's (ADR-0028).
    ///
    /// Two functions rather than one with an `Option`, because a World with no `places.toml` is
    /// **complete** — the pack renders exactly as its author intended — and saying that at the
    /// call site is clearer than passing an absence.
    pub fn project_with(
        chain: &WorldPackChain,
        cast: &[CastMember],
        map: &crate::places::WorldMap,
    ) -> Self {
        let places = resolved(chain, map);

        // Roads join Places by identity, so they resolve against the composed Places rather
        // than against a separate table that could disagree with them.
        let positions: BTreeMap<&str, Placement> = places
            .iter()
            .filter_map(|p| p.placement.map(|pl| (p.id.as_str(), pl)))
            .collect();

        // Nothing here consults the active World about a person. Name and face travel with
        // the character now (ADR-0023), which is why the same crew is recognisably itself in
        // every World it visits.
        let characters = cast
            .iter()
            .map(|member| CharacterView {
                id: member.presence.character.to_string(),
                name: member.name.clone(),
                archetype: member.presence.archetype.id(),
                home: member.home.to_string(),
                place: member.presence.place.to_string(),
                activity: member.presence.activity.clone(),
                action: action_name(member.presence.action),
                class: match member.presence.class {
                    ActivityClass::Idle => "idle",
                    ActivityClass::Work => "work",
                    ActivityClass::Waiting => "waiting",
                },
                mark: member.appearance.as_ref().map(project_mark),
                icon: member.icon.as_ref().map(project_mark),
                actions: member
                    .actions
                    .iter()
                    .map(|(action, mark)| (action_name(*action), project_mark(mark)))
                    .collect(),
                speaks_with: member.speaks_with.clone(),
                sounds_like: member.sounds_like.clone(),
                journey: member.presence.journey.as_ref().map(JourneyView::of),
                routine: member
                    .routine
                    .iter()
                    .map(|(activity, seconds, place)| RoutineStepView {
                        activity: activity.clone(),
                        seconds: *seconds,
                        place: place.as_ref().map(|p| p.to_string()),
                    })
                    .collect(),
            })
            .collect();

        // Roads the user drew, resolved the same way the pack's are: both ends must be
        // placed, and an unplaced end simply means the road is not drawn yet.
        //
        // The endpoints come from the Places so a moved building moves its road. The middle
        // comes only from corners its owner explicitly traced: the World never guesses a bend
        // through terrain nobody described.
        let mut drawn: Vec<RouteView> = Vec::new();
        for road in map.roads.values() {
            let (Some(from), Some(to)) = (
                positions.get(road.from.as_str()),
                positions.get(road.to.as_str()),
            ) else {
                continue;
            };
            let mut points = Vec::with_capacity(road.via.len() + 2);
            points.push([from.x, from.y]);
            points.extend(road.via.iter().copied());
            points.push([to.x, to.y]);
            drawn.push(RouteView {
                points,
                prominence: "major",
            });
        }

        // Two lines between the same two doors is a rendering artefact rather than two roads,
        // so one of them goes - and it is the pack's.
        //
        // It used to be the user's, which is backwards: tracing a road between two Places the
        // pack already joined made the new road silently never appear, and nothing on screen
        // said why. The map belongs to the user (ADR-0028); a shipped World is what they start
        // from, never what overrules them.
        let from_pack: Vec<RouteView> = pack_routes(chain, &positions)
            .into_iter()
            .filter(|r| !drawn.iter().any(|mine| same_ends(r, &mine.points)))
            .collect();

        Self {
            pack_name: chain.active().map(|p| p.name.clone()),
            map: chain
                .map()
                .map(|m| MapView {
                    width: m.size.width,
                    height: m.size.height,
                    terrain: m
                        .terrain
                        .iter()
                        .map(|area| TerrainView {
                            kind: area.kind.id(),
                            points: area.points.iter().map(|p| [p.x, p.y]).collect(),
                        })
                        .collect(),
                    routes: from_pack
                        .iter()
                        .cloned()
                        .chain(drawn.iter().cloned())
                        .collect(),
                })
                // A World whose pack declares no geography still has to have land. Two answers,
                // in order of how much the user said:
                //
                //   1. The image they painted. One pixel, one world unit — so a brand-new World
                //      becomes a real, buildable place the moment they drop a map onto it. This is
                //      what makes the terrain load-bearing rather than decorative: without it an
                //      empty World has no extent, nothing renders, and the first building cannot
                //      be placed anywhere.
                //   2. Otherwise, whatever is standing in it. Derived rendering never goes away
                //      (ADR-0024); it is what is left when nobody authored anything better.
                .or_else(|| {
                    let (width, height) = match map.land_size {
                        Some([w, h]) if w > 0.0 && h > 0.0 => (w, h),
                        _ => extent(&places)?,
                    };
                    Some(MapView {
                        width,
                        height,
                        terrain: Vec::new(),
                        routes: drawn,
                    })
                }),
            places: places.iter().map(project_place).collect(),
            characters,
        }
    }
}

/// Whether two routes join the same two points, in either direction.
///
/// Ends only. A pack route may wander through half the map between the same two doors and it
/// is still the road between those two doors.
fn same_ends(a: &RouteView, b: &[[f32; 2]]) -> bool {
    let (Some(a0), Some(a1)) = (a.points.first(), a.points.last()) else {
        return false;
    };
    let (Some(b0), Some(b1)) = (b.first(), b.last()) else {
        return false;
    };
    (a0 == b0 && a1 == b1) || (a0 == b1 && a1 == b0)
}

/// The pack's own routes, resolved against the composed Places.
fn pack_routes(chain: &WorldPackChain, positions: &BTreeMap<&str, Placement>) -> Vec<RouteView> {
    let Some(m) = chain.map() else {
        return Vec::new();
    };
    m.routes
        .iter()
        .filter_map(|route| {
            // A road needs both ends placed. One missing end means the World is partially
            // authored, so the road is simply not drawn.
            let from = positions.get(route.from.as_str())?;
            let to = positions.get(route.to.as_str())?;
            Some(RouteView {
                points: vec![[from.x, from.y], [to.x, to.y]],
                prominence: match route.prominence {
                    Prominence::Major => "major",
                    Prominence::Minor => "minor",
                },
            })
        })
        .collect()
}

/// How much land a World with no authored geography needs, from what is standing in it.
///
/// `None` when nothing is placed: an empty World has no extent to derive, and inventing one
/// would put a horizon around nothing.
///
/// Generous on purpose — empty land is where future cities go (see `MapView`), and a map that
/// stopped at the last building would make every new one look like it fell off the edge.
fn extent(places: &[Place]) -> Option<(f32, f32)> {
    let mut widest = 0.0f32;
    let mut deepest = 0.0f32;
    let mut any = false;
    for p in places.iter().filter_map(|p| p.placement) {
        any = true;
        widest = widest.max(p.x + p.footprint);
        deepest = deepest.max(p.y + p.footprint);
    }
    any.then_some((widest * 1.4, deepest * 1.4))
}

/// Compose every Place in the World, in draw order.
///
/// Every concept the engine knows appears, whether or not a World described it: a gap must
/// be *visible*, not silently absent.
/// The World's Places, composed and overlaid — what is actually there.
///
/// **One answer, two readers.** The renderer draws these and the Simulation walks between them,
/// and until this was a function the second reader would have had to compose the World a second
/// way. Two compositions is two answers to *where is the Library*, and the day they disagree the
/// disagreement is a character standing somewhere nothing is drawn.
pub fn resolved(chain: &WorldPackChain, map: &crate::places::WorldMap) -> Vec<Place> {
    let mut places = compose_places(chain);
    overlay(&mut places, map);
    places
}

fn compose_places(chain: &WorldPackChain) -> Vec<Place> {
    let mut places: Vec<Place> = chain
        .place_ids()
        .iter()
        .filter_map(|id| place::compose(id, &chain.contributions(id)))
        .collect();

    for concept in PlaceConcept::ALL {
        if !places.iter().any(|p| p.concept == Some(concept)) {
            places.push(Place::placeholder(concept));
        }
    }

    places.sort_by(Place::by_draw_order);
    places
}

/// A Place that exists and nothing else is known about yet.
///
/// Separate from `Place::placeholder`, which is about a *concept* the Engine expects and no
/// World described. This is about a building somebody made.
fn blank(id: crate::place::PlaceId) -> Place {
    let title = id.to_string();
    Place {
        id,
        concept: None,
        title,
        subtitle: None,
        is_placeholder: true,
        placement: None,
        marks: Vec::new(),
        anchors: Vec::new(),
    }
}

/// Lay the user's map over the pack's, in place.
///
/// **Adds; never replaces.** Every field of an entry is optional and an absent one leaves the
/// pack's answer standing — a Place renamed but not re-drawn keeps its artwork, and one the
/// user has never touched is untouched. That is ADR-0024's rule again: authored content wins,
/// derived rendering never goes away.
///
/// A Place the map names that the pack does not have is **created**, because that is how a user
/// builds a World the pack never imagined. It arrives with no placement — it exists, and it is
/// not positioned until somebody positions it, which a partially authored World is allowed to
/// be (see `Place::placement`).
fn overlay(places: &mut Vec<Place>, map: &crate::places::WorldMap) {
    for (id, entry) in &map.places {
        match places.iter_mut().find(|p| &p.id == id) {
            Some(place) => {
                if let Some(name) = &entry.name {
                    place.title = name.clone();
                    // It has a name somebody chose, so it is no longer a stand-in for a Place
                    // nobody described — whatever the pack did or did not say about it.
                    place.is_placeholder = false;
                }
                if let Some(concept) = entry.concept {
                    place.concept = Some(concept);
                }
                if let Some(spot) = entry.spot {
                    place.placement = Some(spotted(spot, place.placement));
                }
                if let Some(art) = &entry.art {
                    redraw(place, art);
                }
            }
            None => {
                // No concept is legal, so this is not `unwrap_or` anything — a Place the
                // Engine cannot animate is still a Place.
                let mut place = blank(id.clone());
                place.concept = entry.concept;
                place.id = id.clone();
                place.title = entry.name.clone().unwrap_or_else(|| id.to_string());
                place.is_placeholder = entry.name.is_none();
                place.placement = entry.spot.map(|spot| spotted(spot, None));
                if let Some(art) = &entry.art {
                    redraw(&mut place, art);
                }
                places.push(place);
            }
        }
    }

    places.sort_by(Place::by_draw_order);
}

/// Put the user's artwork on a Place, in place of whatever the pack drew.
///
/// **Replaces rather than adds**, and it is the one part of the overlay that does. Everywhere
/// else an absent field leaves the pack's answer standing, because absence means "the pack
/// decides". A present drawing is not an absence — it is somebody saying *this is what that
/// building looks like now*, and stacking it over the pack's would render two buildings in one
/// place. Same rule as ADR-0024: authored artwork wins.
///
/// The decorations go with it. They belonged to the building that is no longer there.
fn redraw(place: &mut Place, art: &str) {
    place.marks = vec![crate::place::Mark {
        key: None,
        role: epoch_kernel::MarkRole::Visual,
        renderer: epoch_kernel::RendererKind::Sprite,
        asset: None,
        asset_data: Some(art.to_owned()),
        // Filling the footprint, standing on the ground. A Place's size is its footprint and
        // there is deliberately no second multiplier, so an imported drawing is scaled to the
        // building rather than given a scale of its own to disagree with it.
        scale: 1.0,
        anchor: [0.5, 1.0],
        shape: Vec::new(),
        // An imported drawing is a picture. A user who imports a sheet gets a sheet drawn,
        // which is visible, wrong and theirs to fix — never a guess at how to cut it.
        frames: None,
    }];
}

/// Fold the user's position onto whatever the pack said about this Place.
///
/// The absent parts of a `Spot` fall back to the pack rather than to a constant, which is the
/// same rule as the rest of the overlay one level down: dragging a building the pack drew must
/// not resize it, and it must not reorder it either.
///
/// `z_order` is never authored by a drag. An explicit z beats position (see
/// `Place::by_draw_order`), so letting the editor set one would let a gesture override an
/// intention — and there is no gesture that means "and I meant that".
fn spotted(spot: crate::places::Spot, pack: Option<Placement>) -> Placement {
    Placement {
        x: spot.x,
        y: spot.y,
        footprint: spot
            .footprint
            .or(pack.map(|p| p.footprint))
            .unwrap_or(crate::places::DEFAULT_FOOTPRINT),
        z_order: pack.map(|p| p.z_order).unwrap_or(0),
    }
}

/// One drawn thing, projected. Used by Places and by Characters alike — appearance travels
/// one path out of the Engine, whatever it is the appearance of.
pub fn project_mark(m: &crate::place::Mark) -> MarkView {
    MarkView {
        role: m.role.id(),
        renderer: m.renderer.id(),
        supported: m.is_supported(),
        asset: m.asset_data.clone(),
        scale: m.scale,
        anchor: m.anchor,
        shape: m
            .shape
            .iter()
            .map(|l| ShapeLayerView {
                form: l.form.id(),
                tone: l.tone.id(),
                x: l.x,
                y: l.y,
                w: l.w,
                h: l.h,
                r: l.r,
                points: l.points.clone(),
            })
            .collect(),
        frames: m.frames.as_ref().map(|f| FramesView {
            columns: f.columns,
            rows: f.rows,
            count: f.count,
            milliseconds: f.milliseconds,
            directions: f.directions.iter().map(|d| d.id()).collect(),
        }),
    }
}

fn project_place(place: &Place) -> PlaceView {
    PlaceView {
        id: place.id.to_string(),
        concept: place.concept.map(|c| c.id()),
        title: place.title.clone(),
        subtitle: place.subtitle.clone(),
        is_placeholder: place.is_placeholder,
        placement: place.placement.map(|p| PlacementView {
            x: p.x,
            y: p.y,
            footprint: p.footprint,
            z_order: p.z_order,
        }),
        marks: place.marks.iter().map(project_mark).collect(),
        anchors: place
            .anchors
            .iter()
            .map(|a| AnchorView {
                role: a.role.id(),
                supported: a.role.is_implemented(),
                x: a.x,
                y: a.y,
                w: a.w,
                h: a.h,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use epoch_kernel::{CharacterArchetype, CharacterId, PresenceState};

    fn member(id: &str, name: &str, activity: &str) -> CastMember {
        CastMember {
            home: epoch_kernel::PlaceId::new("research_lab"),
            speaks_with: None,
            sounds_like: None,
            routine: Vec::new(),
            presence: PresenceState::idling(
                CharacterId::new(id).unwrap(),
                CharacterArchetype::Researcher,
                epoch_kernel::PlaceId::new("research_lab"),
                activity,
            ),
            name: name.into(),
            appearance: None,
            icon: None,
            actions: Default::default(),
        }
    }

    #[test]
    fn the_world_renders_even_with_no_world_and_nobody_in_it() {
        // Rule 1: the first frame is always the World. Never a loading screen.
        let view = WorldView::project(&WorldPackChain::default(), &[]);
        assert_eq!(view.places.len(), PlaceConcept::ALL.len());
        assert!(view.places.iter().all(|p| p.is_placeholder));
        assert!(view.places.iter().all(|p| p.placement.is_none()));
        assert!(view.characters.is_empty());
        assert!(view.pack_name.is_none());
    }

    #[test]
    fn an_inhabitant_is_projected_with_their_own_name_and_their_activity() {
        let view = WorldView::project(
            &WorldPackChain::default(),
            &[member("mage", "Elowen", "reading")],
        );

        let c = &view.characters[0];
        assert_eq!(c.id, "mage");
        // No World loaded at all, and she is still herself: a name that no longer depends on
        // the active pack cannot be lost by opening a different World (ADR-0023).
        assert_eq!(c.name, "Elowen");
        assert_eq!(c.archetype, "researcher");
        assert_eq!(c.place, "research_lab");
        assert_eq!(c.activity, "reading");
        assert_eq!(c.class, "idle");
    }

    #[test]
    fn two_characters_of_one_archetype_project_as_two_people() {
        let view = WorldView::project(
            &WorldPackChain::default(),
            &[
                member("mage", "Elowen", "reading"),
                member("paladin", "Bram", "checking the seals"),
            ],
        );
        assert_eq!(view.characters.len(), 2);
        assert_ne!(view.characters[0].id, view.characters[1].id);
        assert_eq!(view.characters[0].archetype, view.characters[1].archetype);
    }

    #[test]
    fn a_place_the_user_built_stands_where_they_put_it() {
        // Before this it was created, listed, named — and drawn nowhere, because a Place with
        // no placement is not rendered. A building you cannot see is not a building.
        let mut map = crate::places::WorldMap::default();
        let forge = map.add(
            "The Forge",
            Some(crate::places::Spot {
                x: 420.0,
                y: 260.0,
                footprint: None,
            }),
        );

        let view = WorldView::project_with(&WorldPackChain::default(), &[], &map);
        let place = view
            .places
            .iter()
            .find(|p| p.id == forge.to_string())
            .unwrap();

        let at = place.placement.expect("it is somewhere");
        assert_eq!((at.x, at.y), (420.0, 260.0));
        assert_eq!(place.title, "The Forge");
    }

    #[test]
    fn a_place_nobody_has_put_anywhere_is_visibly_nowhere() {
        // Honest rather than helpful. Dropping it at the origin would hide unfinished authoring
        // under a building standing in the corner of every World.
        let mut map = crate::places::WorldMap::default();
        let forge = map.add("The Forge", None);

        let view = WorldView::project_with(&WorldPackChain::default(), &[], &map);
        let place = view
            .places
            .iter()
            .find(|p| p.id == forge.to_string())
            .unwrap();

        assert!(place.placement.is_none());
    }

    #[test]
    fn a_road_the_user_drew_is_a_road_the_world_draws() {
        // It was stored and never projected: `MapView.routes` came from the pack alone, so
        // joining two buildings changed a file and nothing else.
        let mut map = crate::places::WorldMap::default();
        let a = map.add(
            "The Forge",
            Some(crate::places::Spot {
                x: 100.0,
                y: 100.0,
                footprint: None,
            }),
        );
        let b = map.add(
            "The Library",
            Some(crate::places::Spot {
                x: 500.0,
                y: 300.0,
                footprint: None,
            }),
        );
        map.connect(a, b);

        let view = WorldView::project_with(&WorldPackChain::default(), &[], &map);
        let geography = view
            .map
            .expect("two placed buildings are land enough to draw");

        assert_eq!(geography.routes.len(), 1);
        assert_eq!(
            geography.routes[0].points,
            vec![[100.0, 100.0], [500.0, 300.0]]
        );
    }

    #[test]
    fn a_traced_road_projects_its_corners_but_not_duplicate_endpoints() {
        let mut map = crate::places::WorldMap::default();
        let a = map.add(
            "The Forge",
            Some(crate::places::Spot {
                x: 100.0,
                y: 100.0,
                footprint: None,
            }),
        );
        let b = map.add(
            "The Library",
            Some(crate::places::Spot {
                x: 500.0,
                y: 300.0,
                footprint: None,
            }),
        );
        map.connect_with_via(a, b, vec![[100.0, 260.0], [360.0, 260.0]]);

        let view = WorldView::project_with(&WorldPackChain::default(), &[], &map);
        assert_eq!(
            view.map.expect("the route projects").routes[0].points,
            vec![
                [100.0, 100.0],
                [100.0, 260.0],
                [360.0, 260.0],
                [500.0, 300.0],
            ]
        );
    }

    /// A World that draws one road between the two buildings a test places.
    fn pack_joining(from: &str, to: &str) -> WorldPackChain {
        WorldPackChain::new(vec![crate::pack::WorldPack::joining(from, to)])
    }

    #[test]
    fn a_road_the_user_traced_replaces_the_one_the_pack_drew() {
        // It was the other way round, and it failed silently: tracing a road between two Places
        // the pack already joined kept the pack's straight line, discarded every corner the
        // user had just drawn, and said nothing. It reads as a save that did not work.
        //
        // Both cannot be drawn - two lines between the same two doors is a rendering artefact
        // rather than two roads - so one goes, and it is the pack's. The map belongs to the
        // user (ADR-0028).
        let mut map = crate::places::WorldMap::default();
        let a = map.add(
            "The Forge",
            Some(crate::places::Spot {
                x: 100.0,
                y: 100.0,
                footprint: None,
            }),
        );
        let b = map.add(
            "The Library",
            Some(crate::places::Spot {
                x: 500.0,
                y: 300.0,
                footprint: None,
            }),
        );
        let chain = pack_joining(a.as_str(), b.as_str());
        map.connect_with_via(a, b, vec![[100.0, 300.0]]);

        let routes = WorldView::project_with(&chain, &[], &map)
            .map
            .expect("the pack declares land")
            .routes;

        assert_eq!(routes.len(), 1, "the same pair must not be drawn twice");
        assert_eq!(
            routes[0].points,
            vec![[100.0, 100.0], [100.0, 300.0], [500.0, 300.0]],
            "the corner the user traced is the road that survives"
        );
    }

    #[test]
    fn a_road_the_pack_drew_still_arrives_where_the_user_traced_none() {
        // The other half of the same rule. Replacing is per pair, not wholesale: drawing one
        // road must not remove the World's own.
        let mut map = crate::places::WorldMap::default();
        let a = map.add(
            "The Forge",
            Some(crate::places::Spot {
                x: 100.0,
                y: 100.0,
                footprint: None,
            }),
        );
        let b = map.add(
            "The Library",
            Some(crate::places::Spot {
                x: 500.0,
                y: 300.0,
                footprint: None,
            }),
        );
        let chain = pack_joining(a.as_str(), b.as_str());

        let routes = WorldView::project_with(&chain, &[], &map)
            .map
            .expect("the pack declares land")
            .routes;

        assert_eq!(routes.len(), 1);
        assert_eq!(
            routes[0].points,
            vec![[100.0, 100.0], [500.0, 300.0]],
            "a pack road runs straight between two real doors; it no longer bends"
        );
    }

    #[test]
    fn a_road_with_one_end_nowhere_is_not_drawn_yet() {
        // The same rule the pack's own routes follow. A partially authored World renders what
        // is finished and says nothing about what is not.
        let mut map = crate::places::WorldMap::default();
        let a = map.add(
            "The Forge",
            Some(crate::places::Spot {
                x: 100.0,
                y: 100.0,
                footprint: None,
            }),
        );
        let b = map.add("The Library", None);
        map.connect(a, b);

        let view = WorldView::project_with(&WorldPackChain::default(), &[], &map);
        assert!(view.map.unwrap().routes.is_empty());
    }

    #[test]
    fn imported_artwork_replaces_what_the_pack_drew_rather_than_stacking_on_it() {
        // The one place the overlay replaces instead of adding. Everywhere else an absent field
        // means "the pack decides"; a present drawing is somebody saying what that building
        // looks like now, and keeping both would render two buildings in one place.
        let mut map = crate::places::WorldMap::default();
        let id = epoch_kernel::PlaceId::new("research_lab");
        map.rename(&id, "El Taller");
        map.places.get_mut(&id).unwrap().art = Some("data:image/png;base64,AAAA".into());

        let view = WorldView::project_with(&WorldPackChain::default(), &[], &map);
        let place = view.places.iter().find(|p| p.id == "research_lab").unwrap();

        assert_eq!(
            place.marks.len(),
            1,
            "one drawing, not the pack's plus theirs"
        );
        assert_eq!(place.marks[0].renderer, "sprite");
        assert_eq!(
            place.marks[0].asset.as_deref(),
            Some("data:image/png;base64,AAAA")
        );
        assert_eq!(
            place.title, "El Taller",
            "and the rest of the overlay still applies"
        );
    }

    #[test]
    fn a_place_with_no_imported_artwork_keeps_whatever_the_pack_drew() {
        // Absence has to keep meaning "the pack decides", or naming a building would silently
        // erase it.
        let mut map = crate::places::WorldMap::default();
        let id = epoch_kernel::PlaceId::new("research_lab");
        map.rename(&id, "El Taller");

        let view = WorldView::project_with(&WorldPackChain::default(), &[], &map);
        let place = view.places.iter().find(|p| p.id == "research_lab").unwrap();

        // The default chain draws nothing at all, so the assertion that matters is that
        // nothing was *invented* on the user's behalf.
        assert!(place.marks.iter().all(|m| m.asset.is_none()));
    }

    #[test]
    fn painted_land_gives_a_world_with_no_geography_a_size() {
        // What makes the terrain load-bearing rather than decorative. Without it a brand-new
        // World has no extent, nothing renders, and the first building cannot be placed
        // anywhere — the editor opens onto a World you cannot start.
        let mut map = crate::places::WorldMap::default();
        map.repaint(Some("land.png".into()), Some([1920.0, 1080.0]));

        let view = WorldView::project_with(&WorldPackChain::default(), &[], &map);
        let land = view.map.expect("the picture is the World");

        assert_eq!((land.width, land.height), (1920.0, 1080.0));
    }

    #[test]
    fn land_that_could_not_be_measured_does_not_invent_a_size() {
        // A wrong extent silently rescales everything standing in the World, so an unmeasurable
        // image falls back to what is actually there rather than to a plausible number.
        let mut map = crate::places::WorldMap::default();
        map.repaint(Some("land.png".into()), None);

        let view = WorldView::project_with(&WorldPackChain::default(), &[], &map);
        assert!(view.map.is_none(), "nothing is standing in it either");
    }

    #[test]
    fn an_empty_world_still_has_no_land_to_draw() {
        // Deriving an extent from nothing would put a horizon around nothing.
        let view = WorldView::project_with(
            &WorldPackChain::default(),
            &[],
            &crate::places::WorldMap::default(),
        );
        assert!(view.map.is_none());
    }

    #[test]
    fn projecting_the_same_world_twice_produces_identical_bytes() {
        // Place = Compose(Contributions), all the way out to the wire.
        let chain = WorldPackChain::default();
        let a = serde_json::to_string(&WorldView::project(&chain, &[])).unwrap();
        let b = serde_json::to_string(&WorldView::project(&chain, &[])).unwrap();
        assert_eq!(a, b);
    }
}
