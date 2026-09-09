//! Places — the composition point of a World (ADR-0021, ADR-0022).
//!
//! A Place is not a sprite and not a building. It is a **stable identity** onto which any
//! number of layers project information: the World Definition, later a theme, a locale, an
//! accessibility profile, a runtime overlay. Each layer contributes only what it knows.
//!
//! ## Three phases, deliberately separate
//!
//! ```text
//! Resolve   impure, I/O, at load   files        -> PlaceContribution (assets already read)
//! Compose   pure                   Contributions -> Place
//! Derive    engine state, per tick Place + reality -> runtime state (not yet implemented)
//! ```
//!
//! [`compose`] is a **pure function**. Given the same ordered contributions it always
//! produces the same Place — structurally, semantically and byte for byte. It reads no
//! clock, no environment, no filesystem and no hash iteration order. That is what makes a
//! Place cacheable, reproducible, testable and inspectable.
//!
//! Anything that could break that determinism is rejected during Resolve, where it can be
//! reported. And the unit of rejection is the **field, never the World**: a bad number
//! costs a placement, a bad role costs a mark, and the World still renders (ADR-0016,
//! Build From Life rule 1).
//!
//! ## Merge semantics
//!
//! Contributions arrive in precedence order, most specific first.
//!
//! - **Scalars replace.** The first contribution that supplies a value wins. Asking "what
//!   is the title?" must have exactly one answer.
//! - **Collections compose.** Marks and anchors are gathered in *reverse* order, so the
//!   base is the foundation and specific layers build on top of it.
//! - **Keyed members replace in place.** A mark with a `key` that already exists overrides
//!   it without moving it: an override changes appearance, not draw order. A mark with no
//!   key is anonymous — purely additive, and deliberately impossible to target.
//! - **Anchors are keyed by role.** Consumers ask "where is the spawn point", so that
//!   question gets one answer; a later contribution declaring `spawn` replaces it.
//!
//! Removing a member (tombstones) is deferred. Keys are what make it expressible later
//! without invalidating a single authored World.

use std::collections::BTreeMap;
use std::path::Path;

use epoch_kernel::{AnchorRole, Direction, MarkRole, PlaceConcept, RendererKind};

// The authored cut lives in the Kernel, because both authors of artwork declare the same thing:
// a World Pack marking a building animated, and a character saying what they look like walking.
// Two shapes would have drifted the first time somebody compared them.
pub use epoch_kernel::Sheet;
use serde::Deserialize;

use crate::map::Point;
use crate::render::ShapeLayer;

/// Footprint used when a World places something without saying how big it is.
pub const DEFAULT_FOOTPRINT: f32 = 90.0;

// A Place's identity now lives in the Kernel (ADR-0028). It was declared here, where it was a
// rendering handle for resolving a pack's declarations — and everything about a character's
// life was keyed on the *concept* instead, which is the defect that ADR resolved. One identity,
// one definition; two would have drifted the first time somebody compared them.
pub use epoch_kernel::PlaceId;

// ---------------------------------------------------------------------------
// Authored declarations — what a World manifest holds
// ---------------------------------------------------------------------------

/// A Place as a World author writes it. Every field optional: a contribution is a **patch**,
/// not a whole object, so any number of them stack.
///
/// Roles and renderer kinds are read as plain strings rather than enums on purpose. A typo
/// in one mark must cost that mark, not make the entire World fail to parse.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlaceDeclaration {
    #[serde(default)]
    pub concept: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub at: Option<Point>,
    #[serde(default)]
    pub footprint: Option<f32>,
    /// Explicit draw order, when position alone gets it wrong. Defaults to 0, which means
    /// "sort me by where I am". Without this, an author who needs a Place drawn in front
    /// would have to move it — lying about geography to fix rendering.
    #[serde(default)]
    pub z_order: Option<i32>,
    #[serde(default)]
    pub marks: Vec<MarkDeclaration>,
    #[serde(default)]
    pub anchors: Vec<AnchorDeclaration>,
}

/// One drawn part of a Place, as authored. A mark *is* a Renderable with a declared purpose.
#[derive(Debug, Clone, Deserialize)]
pub struct MarkDeclaration {
    /// Names this mark so a later contribution can replace it. Omit it and the mark is
    /// anonymous: additive only, and never overridden by accident.
    #[serde(default)]
    pub key: Option<String>,
    pub role: String,
    pub renderer: String,
    /// World-relative path to the Asset this mark draws (ADR-0020).
    #[serde(default)]
    pub asset: Option<String>,
    #[serde(default = "one")]
    pub scale: f32,
    #[serde(default = "bottom_centre")]
    pub anchor: [f32; 2],
    #[serde(default)]
    pub shape: Vec<ShapeLayer>,
    /// How to cut this mark's asset into frames, when the asset is a sheet rather than a
    /// picture. Only meaningful for the `animated_sprite` renderer.
    #[serde(default)]
    pub frames: Option<Sheet>,
}

/// A named point or region, as authored. Coordinates are in footprint units.
#[derive(Debug, Clone, Deserialize)]
pub struct AnchorDeclaration {
    pub role: String,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    /// Half-width, for roles that describe a region rather than a point. `0` means a point.
    #[serde(default)]
    pub w: f32,
    /// Half-height. `0` means a point.
    #[serde(default)]
    pub h: f32,
}

fn one() -> f32 {
    1.0
}

fn bottom_centre() -> [f32; 2] {
    [0.5, 1.0]
}

// ---------------------------------------------------------------------------
// Resolved contributions — what Compose is allowed to see
// ---------------------------------------------------------------------------

/// One drawn part of a Place, validated and with its Asset already read.
#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    pub key: Option<String>,
    pub role: MarkRole,
    pub renderer: RendererKind,
    /// The reference exactly as the World wrote it, kept for reporting.
    pub asset: Option<String>,
    /// The Asset as a `data:` URI, filled during Resolve. Never authored, never read by
    /// Compose — which is what keeps Compose free of I/O.
    pub asset_data: Option<String>,
    pub scale: f32,
    pub anchor: [f32; 2],
    pub shape: Vec<ShapeLayer>,
    /// How to cut the asset, when it is a sheet. `None` is a still picture.
    pub frames: Option<Frames>,
}

/// How a sheet is cut, validated.
///
/// **The cut is the Engine's; the playing is not.** This says how many cells there are, where
/// they are and how long each is shown — facts about the artwork. Which cell is on screen at
/// this millisecond is per-frame presentation, and it stays in the renderer for exactly the
/// reason ADR-0018 drew that line for characters: the Engine owns reality, the UI owns
/// animation.
#[derive(Debug, Clone, PartialEq)]
pub struct Frames {
    pub columns: u32,
    pub rows: u32,
    /// Real cells, in reading order. Always resolved to a number — never `0`, never implied.
    pub count: u32,
    pub milliseconds: u32,
    /// Which direction each row walks, by row index. Empty means the sheet is not directional.
    pub directions: Vec<Direction>,
}

impl Mark {
    /// Whether this build can actually draw this mark, both its role and its renderer.
    pub fn is_supported(&self) -> bool {
        self.role.is_implemented() && self.renderer.is_implemented()
    }
}

/// A named point or region, validated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
    pub role: AnchorRole,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// A patch over a Place identity: only what this layer knows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlaceContribution {
    pub concept: Option<PlaceConcept>,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub at: Option<Point>,
    pub footprint: Option<f32>,
    pub z_order: Option<i32>,
    pub marks: Vec<Mark>,
    pub anchors: Vec<Anchor>,
}

impl PlaceDeclaration {
    /// **Resolve**: validate this declaration and read the Assets it references.
    ///
    /// The impure phase. Anything invalid is dropped and reported, so what comes out is
    /// always safe for a pure, deterministic Compose to consume.
    pub fn resolve(&self, id: &PlaceId, world_dir: &Path) -> (PlaceContribution, Vec<String>) {
        let mut problems = Vec::new();
        let mut note = |msg: String| problems.push(format!("place '{id}': {msg}"));

        let concept = match &self.concept {
            None => None,
            Some(raw) => match PlaceConcept::ALL
                .into_iter()
                .find(|c| c.id() == raw.as_str())
            {
                Some(c) => Some(c),
                None => {
                    note(format!("unknown concept '{raw}'"));
                    None
                }
            },
        };

        // A non-finite coordinate would make draw order undefined, and undefined order is
        // exactly what determinism forbids. Drop the position rather than the Place.
        let at = self.at.filter(|p| {
            let ok = p.x.is_finite() && p.y.is_finite();
            if !ok {
                note("position is not a finite number; the Place stays unplaced".into());
            }
            ok
        });

        let footprint = self.footprint.filter(|f| {
            let ok = f.is_finite() && *f > 0.0;
            if !ok {
                note(format!("footprint {f} is not a positive finite number"));
            }
            ok
        });

        let mut marks: Vec<Mark> = Vec::new();
        for declaration in &self.marks {
            let (resolved, found) = resolve_mark(declaration, world_dir);
            for problem in found {
                note(problem);
            }
            let Some(mark) = resolved else { continue };

            // A key declared twice inside one contribution is an authoring mistake with no
            // sensible silent answer. Last wins, and the author is told.
            let duplicate = match mark.key.clone() {
                Some(key) => marks
                    .iter()
                    .position(|m| m.key.as_deref() == Some(key.as_str()))
                    .map(|i| (i, key)),
                None => None,
            };
            match duplicate {
                Some((i, key)) => {
                    note(format!(
                        "mark key '{key}' declared twice; the later one wins"
                    ));
                    marks[i] = mark;
                }
                None => marks.push(mark),
            }
        }

        let mut anchors: Vec<Anchor> = Vec::new();
        for declaration in &self.anchors {
            let Some(role) = AnchorRole::from_id(&declaration.role) else {
                note(format!("unknown anchor role '{}'", declaration.role));
                continue;
            };
            let anchor = Anchor {
                role,
                x: declaration.x,
                y: declaration.y,
                w: declaration.w,
                h: declaration.h,
            };
            if ![anchor.x, anchor.y, anchor.w, anchor.h]
                .into_iter()
                .all(f32::is_finite)
            {
                note(format!(
                    "anchor '{}' carries a non-finite number",
                    role.id()
                ));
                continue;
            }
            match anchors.iter().position(|a| a.role == role) {
                Some(i) => anchors[i] = anchor,
                None => anchors.push(anchor),
            }
        }

        let contribution = PlaceContribution {
            concept,
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            at,
            footprint,
            z_order: self.z_order,
            marks,
            anchors,
        };

        (contribution, problems)
    }
}

/// **Resolve** one authored mark: validate it, and read the Asset it references.
///
/// Shared by Places and Characters so both get identical validation, identical asset
/// confinement and identical honest degradation. A character's appearance is not a second
/// pipeline — it is the same one, pointed at somebody instead of somewhere.
///
/// Returns `None` when the mark cannot be used at all; problems are always returned, so a
/// bad field costs itself rather than the World.
pub fn resolve_mark(
    declaration: &MarkDeclaration,
    world_dir: &Path,
) -> (Option<Mark>, Vec<String>) {
    let mut problems = Vec::new();

    let Some(role) = MarkRole::from_id(&declaration.role) else {
        problems.push(format!("unknown mark role '{}'", declaration.role));
        return (None, problems);
    };
    let Some(renderer) = RendererKind::from_id(&declaration.renderer) else {
        problems.push(format!("unknown renderer '{}'", declaration.renderer));
        return (None, problems);
    };
    if !declaration.scale.is_finite()
        || !declaration.anchor.iter().all(|v| v.is_finite())
        || !declaration
            .shape
            .iter()
            .flat_map(|l| l.numbers())
            .all(f32::is_finite)
    {
        problems.push(format!(
            "mark '{}' carries a non-finite number",
            declaration.role
        ));
        return (None, problems);
    }

    // Read the Asset now. This is the only I/O in the whole appearance pipeline, and it
    // happens here so that Compose never touches a filesystem.
    let mut asset_data = None;
    if let Some(reference) = declaration.asset.as_deref() {
        match crate::asset::resolve(world_dir, reference) {
            Ok(resolved) => asset_data = Some(resolved.data_uri),
            Err(err) => problems.push(err.to_string()),
        }
    }

    // **A sheet with no grid is not a picture.**
    //
    // Everywhere else in this file a bad field costs the field and the Place still renders. A
    // cut is the exception, and the reason is what the asset *is*: an `animated_sprite` asset
    // is a strip of cells, and drawing it whole is not a degraded version of the animation —
    // it is a different image, one the author never drew. So an animated sprite with no usable
    // cut loses the mark, reported, and the renderer falls back the way it already knows how
    // (ADR-0016). Same argument `resolve_appearance` makes about a character being one mark.
    let frames = match (&declaration.frames, renderer) {
        (Some(authored), RendererKind::AnimatedSprite) => {
            match resolve_frames(authored, &declaration.role) {
                Ok(frames) => Some(frames),
                Err(why) => {
                    problems.push(why);
                    return (None, problems);
                }
            }
        }
        (None, RendererKind::AnimatedSprite) => {
            problems.push(format!(
                "mark '{}' renders as an animated sprite but declares no frames; there is no \
                 way to cut the sheet",
                declaration.role
            ));
            return (None, problems);
        }
        // A cut on a still image is not dangerous, only meaningless — the mark draws exactly
        // as it always did, and the author is told the block did nothing.
        (Some(_), other) => {
            problems.push(format!(
                "mark '{}' declares frames but renders as '{}'; the frames are ignored",
                declaration.role,
                other.id()
            ));
            None
        }
        (None, _) => None,
    };

    let mark = Mark {
        key: declaration.key.clone(),
        role,
        renderer,
        asset: declaration.asset.clone(),
        asset_data,
        scale: declaration.scale,
        anchor: declaration.anchor,
        shape: declaration.shape.clone(),
        frames,
    };
    (Some(mark), problems)
}

/// Validate an authored cut, or say in one sentence why it cannot be drawn.
///
/// `Result` rather than the drop-and-report shape the rest of Resolve uses, because the caller
/// has to *stop* — see the comment at its call site.
fn resolve_frames(authored: &Sheet, role: &str) -> Result<Frames, String> {
    let complain = |what: &str| format!("mark '{role}': {what}");

    if authored.columns == 0 || authored.rows == 0 {
        return Err(complain(&format!(
            "a sheet of {}x{} has no cells; columns and rows must both be at least 1",
            authored.columns, authored.rows
        )));
    }
    // Sixteen bits of cells in each direction is far past any sheet anybody draws, and it is
    // the point where the multiplication below could stop being arithmetic.
    let cells = authored.columns.saturating_mul(authored.rows);

    if authored.milliseconds == 0 {
        return Err(complain(
            "no frame duration; a sheet says how fast it runs, and nothing here may guess",
        ));
    }

    let count = match authored.count {
        // Said once. A sheet that is full of frames does not need to say so twice, and saying
        // it twice is how two numbers start disagreeing.
        0 => cells,
        stated if stated <= cells => stated,
        stated => {
            return Err(complain(&format!(
                "{stated} frames declared, but a {}x{} sheet holds {cells}",
                authored.columns, authored.rows
            )))
        }
    };

    if authored.directions.len() as u32 > authored.rows {
        return Err(complain(&format!(
            "{} directions declared for {} rows",
            authored.directions.len(),
            authored.rows
        )));
    }
    let mut directions = Vec::with_capacity(authored.directions.len());
    for raw in &authored.directions {
        match Direction::from_id(raw) {
            Some(direction) => directions.push(direction),
            // Not a dropped direction: an unknown word means the rows no longer line up with
            // what follows it, and a sheet walking south where it should walk east is worse
            // than one that says it could not be read.
            None => return Err(complain(&format!("unknown direction '{raw}'"))),
        }
    }

    Ok(Frames {
        columns: authored.columns,
        rows: authored.rows,
        count,
        milliseconds: authored.milliseconds,
        directions,
    })
}

// ---------------------------------------------------------------------------
// Characters
// ---------------------------------------------------------------------------

/// **Resolve** a character's authored appearance into a drawable mark.
///
/// Deliberately routed through [`resolve_mark`] rather than reading the file itself. A
/// character's artwork is third-party content exactly as a World's is, and it gets the same
/// path confinement, the same size limit, the same `data:` URI treatment and the same honest
/// degradation. One pipeline, pointed at somebody instead of somewhere.
///
/// `characters_dir` is the vault folder that owns every character, so a sprite can never
/// reach outside it — and a character carries their own face into every World they visit
/// (ADR-0023), which is why this no longer resolves against a World directory.
///
/// One mark rather than a list: a character today is one figure, and a value with one
/// possible shape is not yet a composition (Earn Complexity). It becomes a list when
/// animation needs one, and the field is already a `Mark`, so that change costs nothing.
pub fn resolve_appearance(
    appearance: &epoch_kernel::Appearance,
    characters_dir: &Path,
) -> (Option<Mark>, Vec<String>) {
    // No sprite is a complete answer, not a missing one: they may have an icon instead, or
    // nothing at all, and the World draws a stand-in either way.
    let Some(file) = &appearance.sprite else {
        return (None, Vec::new());
    };

    let (mark, problems) = resolve_mark(
        &MarkDeclaration {
            key: None,
            role: MarkRole::Visual.id().to_owned(),
            renderer: RendererKind::Sprite.id().to_owned(),
            asset: Some(file.clone()),
            // The one narrowing, here, where the renderer needs it. Authored precision stays
            // in the file; drawing precision starts at Resolve.
            scale: appearance.scale as f32,
            anchor: [appearance.anchor[0] as f32, appearance.anchor[1] as f32],
            shape: Vec::new(),
            // A character's art is one still picture today. Per-action sheets are the next
            // step, and they arrive by slot rather than by widening this one call.
            frames: None,
        },
        characters_dir,
    );

    // Where a character differs from a Place, and the only place it does.
    //
    // A Place is several marks: one whose asset could not be read still leaves a building
    // standing, drawn by the others. A character is *this one mark*. Kept as-is it would be a
    // sprite with nothing to draw — an invisible person, which is worse than an absent face
    // because the World would have no idea anything was missing.
    //
    // Dropping it instead reports the problem and hands the renderer `None`, which it already
    // knows how to answer with a visible stand-in.
    match mark {
        Some(mark) if mark.asset_data.is_none() => (None, problems),
        other => (other, problems),
    }
}

/// **Resolve** every action a character has been drawn doing.
///
/// The *identical* pipeline again, and this time it costs nothing to say so: an action's sheet
/// becomes a `MarkDeclaration` with the `animated_sprite` renderer and the authored cut, and
/// [`resolve_mark`] does the rest — the same path confinement, the same size limit, the same
/// `data:` URI, the same rule that a sheet nobody can cut is not a picture.
///
/// An action this build does not know costs that entry and is reported. A file written by a
/// later Epoch must still open in this one, which is why the map is keyed by string on disk.
pub fn resolve_actions(
    appearance: &epoch_kernel::Appearance,
    characters_dir: &Path,
) -> (BTreeMap<epoch_kernel::Action, Mark>, Vec<String>) {
    let mut drawn = BTreeMap::new();
    let mut problems = Vec::new();

    for (raw, art) in &appearance.actions {
        let Some(action) = epoch_kernel::Action::from_id(raw) else {
            problems.push(format!("unknown action '{raw}'"));
            continue;
        };
        let (mark, found) = resolve_mark(
            &MarkDeclaration {
                key: None,
                role: MarkRole::Visual.id().to_owned(),
                renderer: RendererKind::AnimatedSprite.id().to_owned(),
                asset: Some(art.file.clone()),
                // The same geometry as the still sprite, because it is the same person: a walk
                // sheet that stood at a different height would make somebody grow when they
                // set out.
                scale: appearance.scale as f32,
                anchor: [appearance.anchor[0] as f32, appearance.anchor[1] as f32],
                shape: Vec::new(),
                frames: Some(art.cut.clone()),
            },
            characters_dir,
        );
        problems.extend(found.into_iter().map(|p| format!("{raw}: {p}")));

        // Same judgement `resolve_appearance` makes, for the same reason: a mark with nothing
        // to draw is an invisible person, and the fallback chain handles an absent one.
        if let Some(mark) = mark.filter(|m| m.asset_data.is_some()) {
            drawn.insert(action, mark);
        }
    }
    (drawn, problems)
}

/// Resolve a character's icon — the image that identifies them, not the one that walks.
///
/// The *identical* pipeline: same `Mark`, same confinement to the characters folder, same
/// `data:` URI, same honest degradation. It differs from the sprite only in geometry, and only
/// because the two answer different questions — an icon fills a box rather than standing on
/// ground, so it is unscaled and bottom-anchored for the portrait renderer that already exists.
///
/// A second, prettier portrait pipeline would be free to disagree with the World about what
/// somebody looks like, and eventually would.
pub fn resolve_icon(file: &str, characters_dir: &Path) -> (Option<Mark>, Vec<String>) {
    let (mark, problems) = resolve_mark(
        &MarkDeclaration {
            key: None,
            role: MarkRole::Visual.id().to_owned(),
            renderer: RendererKind::Sprite.id().to_owned(),
            asset: Some(file.to_owned()),
            scale: 1.0,
            anchor: [0.5, 1.0],
            shape: Vec::new(),
            // A character's art is one still picture today. Per-action sheets are the next
            // step, and they arrive by slot rather than by widening this one call.
            frames: None,
        },
        characters_dir,
    );

    // Same rule as the sprite: an icon with nothing to draw is worse than no icon, because
    // nothing downstream would know to fall back.
    match mark {
        Some(mark) if mark.asset_data.is_none() => (None, problems),
        other => (other, problems),
    }
}

// ---------------------------------------------------------------------------
// The composed Place
// ---------------------------------------------------------------------------

/// Where a Place sits and how it is ordered against its neighbours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub x: f32,
    pub y: f32,
    /// Extent in world units. This *is* the Place's scale — there is deliberately no second
    /// multiplier, so "how big is this Place" has one answer.
    pub footprint: f32,
    pub z_order: i32,
}

/// A Place, fully composed. The output of a pure function over an ordered list of patches.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub id: PlaceId,
    /// What kind of Place this is, when it is a kind the Engine can animate.
    ///
    /// **Optional** (ADR-0028). A concept says what a Place *projects* — a Laboratory wakes
    /// when a provider is installed — and that behaviour stays. But the user creates Places
    /// now, and "The Forge" is not one of the five the Engine knows. A Place with no concept
    /// exists, is visited, holds inhabitants, and lights up for nobody. That is an honest thing
    /// for a building to be, and requiring a concept would have meant either refusing to let
    /// somebody build it or filing it under the nearest lie.
    pub concept: Option<PlaceConcept>,
    pub title: String,
    pub subtitle: Option<String>,
    /// True when no contribution named this Place and the title is a visible stand-in.
    pub is_placeholder: bool,
    /// `None` when the Place is known but not positioned. It still exists; a partially
    /// authored World degrades rather than breaking.
    pub placement: Option<Placement>,
    pub marks: Vec<Mark>,
    pub anchors: Vec<Anchor>,
}

impl Place {
    /// A Place the engine knows must exist, that no World described.
    ///
    /// Deliberately legible as "nobody supplied this" rather than quietly absent: the first
    /// frame is always the World, gaps included (Build From Life rule 1).
    pub fn placeholder(concept: PlaceConcept) -> Self {
        Self {
            id: PlaceId::new(concept.id()),
            concept: Some(concept),
            title: format!("<{}>", concept.id()),
            subtitle: None,
            is_placeholder: true,
            placement: None,
            marks: Vec::new(),
            anchors: Vec::new(),
        }
    }

    pub fn anchor(&self, role: AnchorRole) -> Option<&Anchor> {
        self.anchors.iter().find(|a| a.role == role)
    }

    /// Marks this build carries but cannot draw, so they are reported rather than silently
    /// missing (ADR-0016).
    pub fn unsupported(&self) -> Vec<String> {
        self.marks
            .iter()
            .filter(|m| !m.is_supported())
            .map(|m| {
                format!(
                    "place '{}' declares a '{}' mark using renderer '{}', which this build cannot draw",
                    self.id,
                    m.role.id(),
                    m.renderer.id()
                )
            })
            .collect()
    }

    /// Draw order: explicit `z_order` first, then position, then identity.
    ///
    /// The identity tiebreak matters. Without it, two Places at the same spot would be
    /// ordered by whatever the input happened to be, and the same World could render
    /// differently between runs. Unplaced Places sort last.
    pub fn by_draw_order(a: &Place, b: &Place) -> std::cmp::Ordering {
        match (a.placement, b.placement) {
            (Some(pa), Some(pb)) => pa
                .z_order
                .cmp(&pb.z_order)
                .then(pa.y.total_cmp(&pb.y))
                .then_with(|| a.id.cmp(&b.id)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.id.cmp(&b.id),
        }
    }
}

/// **Compose**: fold an ordered list of contributions into one Place.
///
/// Pure. No clock, no filesystem, no environment, no hash iteration. `contributions` are in
/// precedence order, most specific first. Returns `None` when nothing said what this Place
/// projects — a Place without a concept has no meaning, and inventing one would be the
/// World claiming a capability that does not exist.
pub fn compose(id: &PlaceId, contributions: &[&PlaceContribution]) -> Option<Place> {
    let concept = contributions.iter().find_map(|c| c.concept);

    let title = contributions.iter().find_map(|c| c.title.clone());
    let is_placeholder = title.is_none();

    // Collections build from the base upward: the last contribution is the foundation, and
    // more specific layers add on top of it.
    let mut marks: Vec<Mark> = Vec::new();
    for contribution in contributions.iter().rev() {
        for mark in &contribution.marks {
            let existing = mark
                .key
                .as_deref()
                .and_then(|k| marks.iter().position(|m| m.key.as_deref() == Some(k)));
            match existing {
                // Replace in place: an override changes what a mark looks like, never where
                // it sits in the draw order.
                Some(i) => marks[i] = mark.clone(),
                None => marks.push(mark.clone()),
            }
        }
    }

    let mut anchors: Vec<Anchor> = Vec::new();
    for contribution in contributions.iter().rev() {
        for anchor in &contribution.anchors {
            match anchors.iter().position(|a| a.role == anchor.role) {
                Some(i) => anchors[i] = *anchor,
                None => anchors.push(*anchor),
            }
        }
    }
    // Anchors have no authored order, so give them a stable one rather than leaving output
    // order dependent on which layer happened to mention them first.
    anchors.sort_by_key(|a| a.role.id());

    let placement = contributions.iter().find_map(|c| c.at).map(|at| Placement {
        x: at.x,
        y: at.y,
        footprint: contributions
            .iter()
            .find_map(|c| c.footprint)
            .unwrap_or(DEFAULT_FOOTPRINT),
        z_order: contributions.iter().find_map(|c| c.z_order).unwrap_or(0),
    });

    Some(Place {
        id: id.clone(),
        concept,
        title: title.unwrap_or_else(|| format!("<{id}>")),
        subtitle: contributions.iter().find_map(|c| c.subtitle.clone()),
        is_placeholder,
        placement,
        marks,
        anchors,
    })
}

/// Every Place a chain of contributors declares, keyed by identity.
///
/// A `BTreeMap` rather than a `HashMap`: Rust randomises hash iteration order per process,
/// so a `HashMap` here would make the same World enumerate its Places differently between
/// runs. Determinism is the property we are optimising for, and this is where it would
/// have leaked first.
pub type PlaceDeclarations = BTreeMap<String, PlaceDeclaration>;

#[cfg(test)]
mod tests {
    use super::*;

    fn mark(key: Option<&str>, role: MarkRole) -> Mark {
        Mark {
            key: key.map(str::to_string),
            role,
            renderer: RendererKind::Shape,
            asset: None,
            asset_data: None,
            scale: 1.0,
            anchor: [0.5, 1.0],
            shape: Vec::new(),
            frames: None,
        }
    }

    /// An authored mark, so the cut can be exercised through the same door a pack uses.
    fn declared(renderer: &str, frames: Option<Sheet>) -> MarkDeclaration {
        MarkDeclaration {
            key: None,
            role: MarkRole::Visual.id().to_owned(),
            renderer: renderer.to_owned(),
            asset: None,
            scale: 1.0,
            anchor: [0.5, 1.0],
            shape: Vec::new(),
            frames,
        }
    }

    fn sheet() -> Sheet {
        Sheet {
            columns: 4,
            rows: 4,
            count: 0,
            milliseconds: 120,
            directions: vec!["south".into(), "west".into(), "east".into(), "north".into()],
        }
    }

    #[test]
    fn a_sheet_is_cut_the_way_the_pack_declared_it() {
        let (mark, problems) = resolve_mark(
            &declared("animated_sprite", Some(sheet())),
            Path::new("nowhere"),
        );
        assert!(problems.is_empty(), "{problems:?}");
        let frames = mark.expect("a mark").frames.expect("a cut");
        assert_eq!(frames.columns, 4);
        assert_eq!(frames.rows, 4);
        // Said once: an unstated count is every cell, so two numbers cannot disagree.
        assert_eq!(frames.count, 16);
        assert_eq!(frames.milliseconds, 120);
        assert_eq!(
            frames.directions,
            vec![
                Direction::South,
                Direction::West,
                Direction::East,
                Direction::North
            ]
        );
    }

    #[test]
    fn a_sheet_with_no_usable_cut_costs_the_mark_rather_than_drawing_the_strip() {
        // The one exception to "reject the field, never the World", and the reason is what the
        // asset *is*: a strip drawn whole is a different image, not a degraded animation.
        for (broken, why) in [
            (
                Sheet {
                    columns: 0,
                    ..sheet()
                },
                "no cells",
            ),
            (
                Sheet {
                    milliseconds: 0,
                    ..sheet()
                },
                "no duration",
            ),
            (
                Sheet {
                    count: 17,
                    ..sheet()
                },
                "more frames than cells",
            ),
            (
                Sheet {
                    directions: vec!["south".into(); 5],
                    ..sheet()
                },
                "more directions than rows",
            ),
            (
                Sheet {
                    directions: vec!["sideways".into()],
                    ..sheet()
                },
                "a direction nobody knows",
            ),
        ] {
            let (mark, problems) =
                resolve_mark(&declared("animated_sprite", Some(broken)), Path::new("."));
            assert!(mark.is_none(), "{why} should cost the mark");
            assert_eq!(problems.len(), 1, "{why} should say so, once");
        }
    }

    #[test]
    fn an_animated_sprite_that_never_says_how_to_cut_it_is_not_drawn() {
        let (mark, problems) = resolve_mark(&declared("animated_sprite", None), Path::new("."));
        assert!(mark.is_none());
        assert!(problems[0].contains("no way to cut"), "{problems:?}");
    }

    #[test]
    fn a_cut_on_a_still_picture_is_ignored_and_said_so() {
        // Meaningless rather than dangerous: the mark draws exactly as it always did.
        let (mark, problems) = resolve_mark(&declared("sprite", Some(sheet())), Path::new("."));
        let mark = mark.expect("still a picture");
        assert!(mark.frames.is_none());
        assert!(problems[0].contains("ignored"), "{problems:?}");
    }

    fn base() -> PlaceContribution {
        PlaceContribution {
            concept: Some(PlaceConcept::ResearchLab),
            title: Some("The Laboratory".into()),
            at: Some(Point { x: 10.0, y: 20.0 }),
            footprint: Some(125.0),
            marks: vec![
                mark(Some("body"), MarkRole::Visual),
                mark(None, MarkRole::Shadow),
            ],
            anchors: vec![Anchor {
                role: AnchorRole::Spawn,
                x: 0.66,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn composing_the_same_contributions_always_produces_the_same_place() {
        // The property the whole design is optimised for: Place = Compose(Contributions).
        let id = PlaceId::new("research_lab");
        let a = base();
        let b = PlaceContribution {
            title: Some("Il Laboratorio".into()),
            ..Default::default()
        };

        let first = compose(&id, &[&b, &a]).unwrap();
        let second = compose(&id, &[&b, &a]).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn scalars_replace_and_the_most_specific_layer_wins() {
        let id = PlaceId::new("research_lab");
        let locale = PlaceContribution {
            title: Some("Il Laboratorio".into()),
            ..Default::default()
        };
        let place = compose(&id, &[&locale, &base()]).unwrap();

        assert_eq!(place.title, "Il Laboratorio");
        // A layer that says nothing about a field leaves the base showing through.
        assert_eq!(place.concept, Some(PlaceConcept::ResearchLab));
        assert_eq!(place.placement.unwrap().footprint, 125.0);
        assert!(!place.is_placeholder);
    }

    #[test]
    fn collections_compose_rather_than_overwrite() {
        let id = PlaceId::new("research_lab");
        let theme = PlaceContribution {
            marks: vec![mark(None, MarkRole::Decoration)],
            ..Default::default()
        };
        let place = compose(&id, &[&theme, &base()]).unwrap();

        // The base's two marks survive; the theme adds a third rather than replacing them.
        assert_eq!(place.marks.len(), 3);
        assert_eq!(place.marks[0].role, MarkRole::Visual);
        assert_eq!(place.marks[2].role, MarkRole::Decoration);
    }

    #[test]
    fn a_keyed_mark_replaces_in_place_without_changing_draw_order() {
        let id = PlaceId::new("research_lab");
        let mut replacement = mark(Some("body"), MarkRole::Visual);
        replacement.renderer = RendererKind::Sprite;
        let theme = PlaceContribution {
            marks: vec![replacement],
            ..Default::default()
        };
        let place = compose(&id, &[&theme, &base()]).unwrap();

        assert_eq!(place.marks.len(), 2, "an override must not add a mark");
        assert_eq!(place.marks[0].renderer, RendererKind::Sprite);
        // Still first: overriding appearance must not silently reorder the composition.
        assert_eq!(place.marks[0].key.as_deref(), Some("body"));
    }

    #[test]
    fn an_anonymous_mark_can_never_be_targeted() {
        let id = PlaceId::new("research_lab");
        let theme = PlaceContribution {
            marks: vec![mark(None, MarkRole::Shadow)],
            ..Default::default()
        };
        let place = compose(&id, &[&theme, &base()]).unwrap();

        // Two shadows, because neither is addressable. Additive by construction.
        assert_eq!(
            place
                .marks
                .iter()
                .filter(|m| m.role == MarkRole::Shadow)
                .count(),
            2
        );
    }

    #[test]
    fn an_anchor_is_keyed_by_role_because_consumers_ask_by_role() {
        let id = PlaceId::new("research_lab");
        let theme = PlaceContribution {
            anchors: vec![Anchor {
                role: AnchorRole::Spawn,
                x: -0.5,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            }],
            ..Default::default()
        };
        let place = compose(&id, &[&theme, &base()]).unwrap();

        assert_eq!(
            place.anchors.len(),
            1,
            "'where is the spawn' has one answer"
        );
        assert_eq!(place.anchor(AnchorRole::Spawn).unwrap().x, -0.5);
    }

    #[test]
    fn a_place_nobody_named_is_a_visible_placeholder_rather_than_a_blank() {
        let id = PlaceId::new("research_lab");
        let unnamed = PlaceContribution {
            concept: Some(PlaceConcept::ResearchLab),
            ..Default::default()
        };
        let place = compose(&id, &[&unnamed]).unwrap();
        assert!(place.is_placeholder);
        assert_eq!(place.title, "<research_lab>");
    }

    #[test]
    fn a_place_with_no_concept_exists_and_projects_nothing() {
        // **This rule was reversed on purpose** (ADR-0028). It used to read "a Place with no
        // concept does not exist", on the argument that a Place is the projection of a
        // capability and there was nothing to project. That was right while a pack supplied the
        // map: the Engine knew five kinds of building and every Place was one of them.
        //
        // The user builds the map now, and "The Forge" is not one of the five and never will
        // be. Refusing it would mean either not letting somebody build it, or filing it under
        // the nearest concept that is not what they meant. So it exists, and it lights up for
        // nobody — which is true, and says so.
        let id = PlaceId::new("the_forge");
        let authored = PlaceContribution {
            title: Some("The Forge".into()),
            ..Default::default()
        };
        let place = compose(&id, &[&authored]).expect("a building somebody made");

        assert_eq!(place.title, "The Forge");
        assert!(
            place.concept.is_none(),
            "and no subsystem has any claim on it"
        );
    }

    fn placed(id: &str, y: f32, z_order: i32) -> Place {
        let mut place = Place::placeholder(PlaceConcept::Guild);
        place.id = PlaceId::new(id);
        place.placement = Some(Placement {
            x: 0.0,
            y,
            footprint: 100.0,
            z_order,
        });
        place
    }

    fn ids(places: &[Place]) -> Vec<String> {
        places.iter().map(|p| p.id.to_string()).collect()
    }

    #[test]
    fn draw_order_never_depends_on_input_order() {
        let mut forward = vec![
            placed("a", 10.0, 0),
            placed("b", 10.0, 0),
            placed("c", 5.0, 0),
        ];
        let mut backward = vec![
            placed("c", 5.0, 0),
            placed("b", 10.0, 0),
            placed("a", 10.0, 0),
        ];
        forward.sort_by(Place::by_draw_order);
        backward.sort_by(Place::by_draw_order);

        // Two Places at the same y are broken apart by identity, not by luck.
        assert_eq!(ids(&forward), ids(&backward));
        assert_eq!(ids(&forward), vec!["c", "a", "b"]);
    }

    #[test]
    fn an_explicit_z_order_beats_position() {
        let mut places = [placed("near", 900.0, 0), placed("lifted", 100.0, 1)];
        places.sort_by(Place::by_draw_order);
        // Without z_order the far Place would draw first. The author overrode that without
        // having to move the building — without lying about geography to fix rendering.
        assert_eq!(places[1].id.as_str(), "lifted");
    }

    #[test]
    fn an_unplaced_place_sorts_last_but_still_exists() {
        let mut places = [
            Place::placeholder(PlaceConcept::Guild),
            placed("somewhere", 10.0, 0),
        ];
        places.sort_by(Place::by_draw_order);
        assert_eq!(places[0].id.as_str(), "somewhere");
        assert!(places[1].placement.is_none());
    }

    #[test]
    fn resolve_drops_a_bad_field_and_keeps_the_place() {
        // "Reject the field, not the World."
        let declaration: PlaceDeclaration = toml::from_str(
            r#"
concept = "research_lab"
title = "The Laboratory"
at = { x = 10.0, y = 20.0 }

[[marks]]
role = "visual"
renderer = "shape"

[[marks]]
role = "hologram"
renderer = "shape"

[[anchors]]
role = "spawn"
x = 0.66

[[anchors]]
role = "teleporter"
"#,
        )
        .unwrap();

        let (contribution, problems) =
            declaration.resolve(&PlaceId::new("research_lab"), Path::new("."));

        assert_eq!(
            contribution.marks.len(),
            1,
            "the bad mark went, the good one stayed"
        );
        assert_eq!(contribution.anchors.len(), 1);
        assert_eq!(contribution.title.as_deref(), Some("The Laboratory"));
        assert_eq!(problems.len(), 2, "and both were reported, not swallowed");
        assert!(problems.iter().any(|p| p.contains("hologram")));
        assert!(problems.iter().any(|p| p.contains("teleporter")));
    }

    #[test]
    fn a_non_finite_position_costs_the_position_not_the_place() {
        let declaration: PlaceDeclaration = toml::from_str(
            r#"
concept = "guild"
title = "The Guild"
at = { x = nan, y = 20.0 }
"#,
        )
        .unwrap();

        let (contribution, problems) = declaration.resolve(&PlaceId::new("guild"), Path::new("."));
        assert!(
            contribution.at.is_none(),
            "undefined order must never reach Compose"
        );
        assert_eq!(contribution.title.as_deref(), Some("The Guild"));
        assert_eq!(problems.len(), 1);
    }
}
