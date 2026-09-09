//! The primitives a `shape` Renderable is built from (ADR-0019).
//!
//! The engine carries these without understanding them. It never learns what a Laboratory
//! looks like; it only learns that the active World drew some layers. Changing a building's
//! appearance is editing a World: no Rust, no React, no engine change.
//!
//! ## Coordinates
//!
//! Geometry is in **footprint units**: `1.0` is the Place's footprint, the origin is the
//! base centre, and `y` grows downward as everywhere else. Geometry therefore scales with
//! whatever footprint the World authored, and the same shape can be reused at any size.

use serde::{Deserialize, Serialize};

/// Which colour role a layer takes. Roles, not colours: the World may later declare a
/// palette, and lighting (an inhabited place) can shift a role without editing geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Wall,
    Roof,
    Detail,
    Accent,
    /// Contact shadow. A colour role like any other, so a World can restyle its shadows
    /// without the renderer knowing what a shadow is.
    Shadow,
}

impl Tone {
    pub const fn id(self) -> &'static str {
        match self {
            Tone::Wall => "wall",
            Tone::Roof => "roof",
            Tone::Detail => "detail",
            Tone::Accent => "accent",
            Tone::Shadow => "shadow",
        }
    }
}

/// The primitive a layer draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Form {
    Rect,
    Polygon,
    Dome,
    /// Centre at `(x, y)`, radii `w` and `h` — centre-based like [`Form::Dome`], because
    /// the things it draws (shadows, pools of light) are naturally described from a centre.
    Ellipse,
}

impl Form {
    pub const fn id(self) -> &'static str {
        match self {
            Form::Rect => "rect",
            Form::Polygon => "polygon",
            Form::Dome => "dome",
            Form::Ellipse => "ellipse",
        }
    }
}

/// One drawn layer of a `shape` Renderable, in declaration order.
///
/// Geometry fields are flat and optional rather than nested per form. That keeps the TOML a
/// World author writes readable, and keeps the wire shape trivial for the renderer to switch
/// on. The cost is that irrelevant fields are simply ignored — an acceptable trade for a
/// format humans author by hand.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ShapeLayer {
    pub form: Form,
    pub tone: Tone,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    /// Width for `rect`, x-radius for `ellipse`.
    #[serde(default)]
    pub w: f32,
    /// Height for `rect`, y-radius for `ellipse`.
    #[serde(default)]
    pub h: f32,
    /// Corner radius for `rect`, or radius for `dome`.
    #[serde(default)]
    pub r: f32,
    /// Outline for `polygon`, in footprint units.
    #[serde(default)]
    pub points: Vec<[f32; 2]>,
}

impl ShapeLayer {
    /// Every geometry number this layer carries, for validation.
    ///
    /// Composition must be deterministic, and an authored `nan` makes ordering and geometry
    /// undefined. Rather than let that reach the renderer, the loader checks here.
    pub fn numbers(&self) -> impl Iterator<Item = f32> + '_ {
        [self.x, self.y, self.w, self.h, self.r]
            .into_iter()
            .chain(self.points.iter().flat_map(|p| [p[0], p[1]]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shape_layer_parses_with_only_the_fields_its_form_needs() {
        let layer: ShapeLayer = toml::from_str(
            r#"form = "dome"
tone = "roof"
r = 0.43
"#,
        )
        .unwrap();
        assert_eq!(layer.form, Form::Dome);
        assert_eq!(layer.tone, Tone::Roof);
        assert_eq!(layer.r, 0.43);
        // Unused geometry defaults rather than forcing the author to write zeroes.
        assert_eq!(layer.w, 0.0);
        assert!(layer.points.is_empty());
    }

    #[test]
    fn every_form_and_tone_is_declarable_so_manifests_never_become_invalid() {
        for form in ["rect", "polygon", "dome", "ellipse"] {
            let l: ShapeLayer =
                toml::from_str(&format!("form = \"{form}\"\ntone = \"wall\"\n")).unwrap();
            assert_eq!(l.form.id(), form);
        }
        for tone in ["wall", "roof", "detail", "accent", "shadow"] {
            let l: ShapeLayer =
                toml::from_str(&format!("form = \"rect\"\ntone = \"{tone}\"\n")).unwrap();
            assert_eq!(l.tone.id(), tone);
        }
    }
}
