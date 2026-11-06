//! Geometry for a math formula: metrics traits, atom classes, style levels and
//! the display list a host paints.
//!
//! Lays out the model tree: rows, scripts, fractions, radicals, delimiters and
//! large operators. Every node carries the source span the parser gave it;
//! editing geometry arrives in a later milestone.

pub mod atom;
pub mod class;
pub mod color;
pub mod display;
pub mod font;
pub mod geometry;
pub mod layout;
pub mod node;
pub mod span;
pub mod style;
pub mod text;

pub use atom::{Atom, Variant, math_italic, variant_char};
pub use class::{AtomClass, Gap, gap};
pub use color::Color;
pub use display::{GlyphItem, Item, Layout, LineItem, RuleItem};
pub use font::{Constants, GlyphId, GlyphOutline, InkBounds, MathFont, PathSink, Stretched};
pub use geometry::{CaretPath, CaretRect, EmptySlot, Geometry, Point, Rect, Step};
pub use layout::{
    Diagnostic, layout, layout_with_geometry, layout_with_shaper, layout_with_shaper_and_geometry,
};
pub use node::{
    Alignment, ChildSlot, ColumnSpacing, DelimiterSize, EditError, FractionRule, Length,
    LengthUnit, Limits, Model, ModelBuilder, Node, NodeId, NodeKind, Origin, PhantomKind, Slot,
    SpaceKind,
};
pub use span::Span;
pub use style::{Size, Style};
pub use text::{FontShaper, ShapedGlyph, ShapedRun, TextShaper, TextStyle};
