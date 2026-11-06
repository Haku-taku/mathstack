//! The flat output a host paints.
//!
//! Coordinates are the caller's units, y down from the formula's top edge; a
//! glyph's `baseline` is the absolute baseline y, and `size` is its em size.

use crate::color::Color;
use crate::font::GlyphId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Item {
    Glyph(GlyphItem),
    Rule(RuleItem),
    /// A stroked segment, such as a cancellation diagonal.
    Line(LineItem),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphItem {
    pub glyph: GlyphId,
    pub x: f32,
    pub baseline: f32,
    pub size: f32,
    /// The color of the enclosing `\color`/`\textcolor`, if any.
    pub color: Option<Color>,
}

/// A filled rectangle, such as a fraction bar; `y` is its top edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuleItem {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: Option<Color>,
}

/// A stroked segment from `(x1, y1)` to `(x2, y2)`, in caller coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineItem {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub thickness: f32,
    pub color: Option<Color>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
    pub items: Vec<Item>,
}
