//! Host text shaping.
//!
//! The engine owns math layout, not font fallback or combining-mark
//! positioning. A host measures `\text`, `\operatorname` and the IME preedit
//! through [`TextShaper`]; [`FontShaper`] is the trivial fallback that reads
//! the same face one glyph per character.

use crate::font::{GlyphId, MathFont};

/// What the host needs to know about the text it is shaping.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextStyle {
    pub bold: bool,
    pub italic: bool,
}

/// One positioned glyph in a shaped run. `x` and `y` are em relative to the
/// run's start and baseline; `y` points up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapedGlyph {
    pub glyph: GlyphId,
    pub x: f32,
    pub y: f32,
    pub size: f32,
}

/// A host-measured text run, in em units.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShapedRun {
    pub glyphs: Vec<ShapedGlyph>,
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
    /// Characters the host had no glyph for; layout reports them at the
    /// construct's span.
    pub missing: Vec<char>,
}

pub trait TextShaper {
    fn shape(&self, text: &str, style: TextStyle) -> ShapedRun;
}

/// The fallback shaper: one glyph per character, the face's own advances and
/// ink. It has no fallback font and no combining-mark placement, which is why
/// a host that cares about those installs its own.
pub struct FontShaper<'a> {
    font: &'a dyn MathFont,
}

impl<'a> FontShaper<'a> {
    pub fn new(font: &'a dyn MathFont) -> Self {
        FontShaper { font }
    }
}

impl TextShaper for FontShaper<'_> {
    fn shape(&self, text: &str, _style: TextStyle) -> ShapedRun {
        let mut run = ShapedRun::default();
        for symbol in text.chars() {
            let Some(glyph) = self.font.glyph(symbol) else {
                run.missing.push(symbol);
                continue;
            };
            if let Some(ink) = self.font.ink(glyph) {
                run.ascent = run.ascent.max(ink.top);
                run.descent = run.descent.max((-ink.bottom).max(0.0));
            }
            run.glyphs.push(ShapedGlyph {
                glyph,
                x: run.width,
                y: 0.0,
                size: 1.0,
            });
            run.width += self.font.advance(glyph);
        }
        run
    }
}
