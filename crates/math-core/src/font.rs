//! What layout asks of a font, and nothing about SFNT.
//!
//! Lengths are in em (an em is the font size), y points up, so ink above the
//! baseline has positive `top`.

/// An SFNT glyph index, not a codepoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlyphId(pub u16);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InkBounds {
    pub left: f32,
    pub bottom: f32,
    pub right: f32,
    pub top: f32,
}

/// The MATH constants layout consumes, in em.
///
/// MATH records them in design units without a style level; a consumer scales
/// them by the em size of the style it is laying out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Constants {
    pub script_percent_scale_down: f32,
    pub script_script_percent_scale_down: f32,
    pub axis_height: f32,
    pub fraction_rule_thickness: f32,
    pub radical_rule_thickness: f32,
    pub space_after_script: f32,
    pub superscript_shift_up: f32,
    pub superscript_shift_up_cramped: f32,
    pub superscript_baseline_drop_max: f32,
    pub subscript_shift_down: f32,
    pub subscript_baseline_drop_min: f32,
    pub sub_superscript_gap_min: f32,
    pub fraction_numerator_shift_up: f32,
    pub fraction_denominator_shift_down: f32,
    pub fraction_numerator_display_shift_up: f32,
    pub fraction_denominator_display_shift_down: f32,
    pub fraction_numerator_gap_min: f32,
    pub fraction_denominator_gap_min: f32,
    pub fraction_numerator_display_gap_min: f32,
    pub fraction_denominator_display_gap_min: f32,
    pub stack_top_shift_up: f32,
    pub stack_bottom_shift_down: f32,
    pub stack_gap_min: f32,
    pub stack_top_display_shift_up: f32,
    pub stack_bottom_display_shift_down: f32,
    pub stack_display_gap_min: f32,
    pub radical_vertical_gap: f32,
    pub radical_display_vertical_gap: f32,
    pub radical_extra_ascender: f32,
    pub radical_kern_before_degree: f32,
    pub radical_kern_after_degree: f32,
    /// Fraction of the radical height, already divided by 100.
    pub radical_degree_bottom_raise: f32,
    pub overbar_vertical_gap: f32,
    pub overbar_rule_thickness: f32,
    pub overbar_extra_ascender: f32,
    pub underbar_vertical_gap: f32,
    pub underbar_rule_thickness: f32,
    pub underbar_extra_descender: f32,
    /// Where an accent glyph's baseline sits for a base of this height; taller
    /// bases push the accent up by the excess.
    pub accent_base_height: f32,
    pub display_operator_min_height: f32,
    pub upper_limit_gap_min: f32,
    pub upper_limit_baseline_rise_min: f32,
    pub lower_limit_gap_min: f32,
    pub lower_limit_baseline_drop_min: f32,
}

impl Default for Constants {
    /// TeX-like fallbacks for a face without MATH, and for tests that only
    /// need plausible metrics.
    fn default() -> Self {
        Constants {
            script_percent_scale_down: 0.7,
            script_script_percent_scale_down: 0.49,
            axis_height: 0.25,
            fraction_rule_thickness: 0.04,
            radical_rule_thickness: 0.04,
            space_after_script: 0.05,
            superscript_shift_up: 0.4,
            superscript_shift_up_cramped: 0.3,
            superscript_baseline_drop_max: 0.3,
            subscript_shift_down: 0.2,
            subscript_baseline_drop_min: 0.1,
            sub_superscript_gap_min: 0.2,
            fraction_numerator_shift_up: 0.4,
            fraction_denominator_shift_down: 0.3,
            fraction_numerator_display_shift_up: 0.6,
            fraction_denominator_display_shift_down: 0.5,
            fraction_numerator_gap_min: 0.1,
            fraction_denominator_gap_min: 0.1,
            fraction_numerator_display_gap_min: 0.2,
            fraction_denominator_display_gap_min: 0.2,
            stack_top_shift_up: 0.3,
            stack_bottom_shift_down: 0.3,
            stack_gap_min: 0.12,
            stack_top_display_shift_up: 0.4,
            stack_bottom_display_shift_down: 0.4,
            stack_display_gap_min: 0.2,
            radical_vertical_gap: 0.1,
            radical_display_vertical_gap: 0.2,
            radical_extra_ascender: 0.05,
            radical_kern_before_degree: 0.05,
            radical_kern_after_degree: 0.1,
            radical_degree_bottom_raise: 0.6,
            overbar_vertical_gap: 0.1,
            overbar_rule_thickness: 0.04,
            overbar_extra_ascender: 0.1,
            underbar_vertical_gap: 0.1,
            underbar_rule_thickness: 0.04,
            underbar_extra_descender: 0.1,
            accent_base_height: 0.5,
            display_operator_min_height: 1.2,
            upper_limit_gap_min: 0.1,
            upper_limit_baseline_rise_min: 0.2,
            lower_limit_gap_min: 0.1,
            lower_limit_baseline_drop_min: 0.2,
        }
    }
}

/// A glyph grown along an axis, either as a pre-drawn variant or as stacked
/// parts. Distances are em from the bottom (vertical) or start (horizontal).
#[derive(Clone, Debug, PartialEq)]
pub enum Stretched {
    Glyph(GlyphId),
    Assembly {
        parts: Vec<(GlyphId, f32)>,
        extent: f32,
    },
}

pub trait MathFont {
    /// The glyph for a codepoint through the face's cmap.
    fn glyph(&self, symbol: char) -> Option<GlyphId>;

    /// Horizontal advance in em.
    fn advance(&self, glyph: GlyphId) -> f32;

    /// Ink box in em; `None` for glyphs without outlines, such as a space.
    fn ink(&self, glyph: GlyphId) -> Option<InkBounds>;

    fn constants(&self) -> Constants;

    /// Italic correction in em; zero when the face has none.
    fn italic(&self, _glyph: GlyphId) -> f32 {
        0.0
    }

    /// MATH top-accent attachment in em: where an accent centres over this
    /// glyph. Faces without the table return `None`, so accents use the box
    /// centre instead.
    fn top_accent_attachment(&self, _glyph: GlyphId) -> Option<f32> {
        None
    }

    /// Fallback ascent and descent in em, used for glyphs without ink.
    fn vertical_metrics(&self) -> (f32, f32) {
        (0.8, 0.2)
    }

    /// x-height in em, used by the script and radical rules.
    fn x_height(&self) -> f32 {
        0.5
    }

    /// Grows `glyph` vertically to at least `target` em; faces without MATH
    /// variants return the glyph unchanged.
    fn stretch_vertical(&self, glyph: GlyphId, _target: f32) -> Stretched {
        Stretched::Glyph(glyph)
    }

    /// Grows `glyph` horizontally to at least `target` em.
    fn stretch_horizontal(&self, glyph: GlyphId, _target: f32) -> Stretched {
        Stretched::Glyph(glyph)
    }

    /// The font's `ssty` alternate for `glyph` at script depth 1 or 2.
    fn script_alternate(&self, _glyph: GlyphId, _level: u8) -> Option<GlyphId> {
        None
    }
}

/// Outlines are separate from metrics so a host that only measures never pays
/// for path extraction.
pub trait GlyphOutline: MathFont {
    /// Feeds the glyph outline in em, y up, to `sink`; false when the face has
    /// no outline for it.
    fn outline(&self, glyph: GlyphId, sink: &mut dyn PathSink) -> bool;
}

pub trait PathSink {
    fn move_to(&mut self, x: f32, y: f32);
    fn line_to(&mut self, x: f32, y: f32);
    fn quad_to(&mut self, control_x: f32, control_y: f32, x: f32, y: f32);
    fn curve_to(
        &mut self,
        control1_x: f32,
        control1_y: f32,
        control2_x: f32,
        control2_y: f32,
        x: f32,
        y: f32,
    );
    fn close(&mut self);
}
