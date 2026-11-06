//! A `MathFont` over one SFNT face, with metrics read from the font.
//!
//! ttf-parser supplies the SFNT and MATH structures; every length crossing this
//! boundary is converted to em once, so layout never sees design units.

use math_core::{Constants, GlyphId, GlyphOutline, InkBounds, MathFont, PathSink, Stretched};
use ttf_parser::{
    Face as SfntFace, GlyphId as SfntGlyphId, OutlineBuilder, Tag,
    gsub::{AlternateSubstitution, SubstitutionSubtable},
    math::GlyphPart,
    math::MathValue,
};

pub struct Face<'a> {
    face: SfntFace<'a>,
    upem: f32,
    constants: Constants,
    /// The `ssty` alternate subtables, in feature order.
    ssty: Vec<AlternateSubstitution<'a>>,
}

impl<'a> Face<'a> {
    /// Parses face `index` from `data`; the bytes must outlive the face.
    pub fn parse(data: &'a [u8], index: u32) -> Result<Self, ttf_parser::FaceParsingError> {
        let face = SfntFace::parse(data, index)?;
        let upem = f32::from(face.units_per_em());
        let constants = read_constants(&face, upem);
        let ssty = read_ssty(&face);
        Ok(Face {
            face,
            upem,
            constants,
            ssty,
        })
    }
}

impl MathFont for Face<'_> {
    fn glyph(&self, symbol: char) -> Option<GlyphId> {
        self.face.glyph_index(symbol).map(|glyph| GlyphId(glyph.0))
    }

    fn advance(&self, glyph: GlyphId) -> f32 {
        self.face
            .glyph_hor_advance(SfntGlyphId(glyph.0))
            .map_or(0.0, |advance| f32::from(advance) / self.upem)
    }

    fn ink(&self, glyph: GlyphId) -> Option<InkBounds> {
        let bounds = self.face.glyph_bounding_box(SfntGlyphId(glyph.0))?;
        Some(InkBounds {
            left: f32::from(bounds.x_min) / self.upem,
            bottom: f32::from(bounds.y_min) / self.upem,
            right: f32::from(bounds.x_max) / self.upem,
            top: f32::from(bounds.y_max) / self.upem,
        })
    }

    fn constants(&self) -> Constants {
        self.constants
    }

    fn italic(&self, glyph: GlyphId) -> f32 {
        let info = self.face.tables().math.and_then(|tables| tables.glyph_info);
        let Some(corrections) = info.and_then(|info| info.italic_corrections) else {
            return 0.0;
        };
        corrections
            .get(SfntGlyphId(glyph.0))
            .map_or(0.0, |value| f32::from(value.value) / self.upem)
    }

    fn top_accent_attachment(&self, glyph: GlyphId) -> Option<f32> {
        let info = self.face.tables().math.and_then(|tables| tables.glyph_info);
        let attachments = info.and_then(|info| info.top_accent_attachments)?;
        let value = attachments.get(SfntGlyphId(glyph.0))?;
        Some(f32::from(value.value) / self.upem)
    }

    fn vertical_metrics(&self) -> (f32, f32) {
        (
            f32::from(self.face.ascender()) / self.upem,
            -f32::from(self.face.descender()) / self.upem,
        )
    }

    fn x_height(&self) -> f32 {
        self.face
            .x_height()
            .map(|height| f32::from(height) / self.upem)
            .filter(|height| *height > 0.0)
            .unwrap_or(0.5)
    }

    fn script_alternate(&self, glyph: GlyphId, level: u8) -> Option<GlyphId> {
        for alternate in &self.ssty {
            let Some(index) = alternate.coverage.get(SfntGlyphId(glyph.0)) else {
                continue;
            };
            let Some(set) = alternate.alternate_sets.get(index) else {
                continue;
            };
            let Some(last) = set.alternates.len().checked_sub(1) else {
                continue;
            };
            let pick = u16::from(level.saturating_sub(1)).min(last);
            return set.alternates.get(pick).map(|glyph| GlyphId(glyph.0));
        }
        None
    }

    /// Formulary's three-step protocol: smallest fitting variant, then an
    /// assembly with uniform overlap, then the largest variant (MIT; see
    /// `THIRD-PARTY.md`).
    fn stretch_vertical(&self, glyph: GlyphId, target: f32) -> Stretched {
        let Some(variants) = self.face.tables().math.and_then(|tables| tables.variants) else {
            return Stretched::Glyph(glyph);
        };
        let Some(construction) = variants.vertical_constructions.get(SfntGlyphId(glyph.0)) else {
            return Stretched::Glyph(glyph);
        };
        let mut best = glyph;
        for variant in construction.variants {
            best = GlyphId(variant.variant_glyph.0);
            if f32::from(variant.advance_measurement) / self.upem >= target {
                return Stretched::Glyph(best);
            }
        }
        construction
            .assembly
            .and_then(|assembly| {
                self.assemble(
                    assembly.parts.into_iter().collect(),
                    f32::from(variants.min_connector_overlap) / self.upem,
                    target,
                )
            })
            .unwrap_or(Stretched::Glyph(best))
    }

    /// The horizontal counterpart of `stretch_vertical`; part offsets count
    /// from the left end.
    fn stretch_horizontal(&self, glyph: GlyphId, target: f32) -> Stretched {
        let Some(variants) = self.face.tables().math.and_then(|tables| tables.variants) else {
            return Stretched::Glyph(glyph);
        };
        let Some(construction) = variants.horizontal_constructions.get(SfntGlyphId(glyph.0)) else {
            return Stretched::Glyph(glyph);
        };
        let mut best = glyph;
        for variant in construction.variants {
            best = GlyphId(variant.variant_glyph.0);
            if f32::from(variant.advance_measurement) / self.upem >= target {
                return Stretched::Glyph(best);
            }
        }
        construction
            .assembly
            .and_then(|assembly| {
                self.assemble(
                    assembly.parts.into_iter().collect(),
                    f32::from(variants.min_connector_overlap) / self.upem,
                    target,
                )
            })
            .unwrap_or(Stretched::Glyph(best))
    }
}

impl Face<'_> {
    /// Stacks assembly parts to reach `target`, repeating extenders and
    /// spreading a uniform overlap between the connectors.
    fn assemble(&self, parts: Vec<GlyphPart>, min_overlap: f32, target: f32) -> Option<Stretched> {
        for repeats in 1..=32u32 {
            let mut sequence: Vec<GlyphPart> = Vec::new();
            for part in &parts {
                let count = if part.part_flags.extender() {
                    repeats
                } else {
                    1
                };
                for _ in 0..count {
                    sequence.push(*part);
                }
            }
            if sequence.len() < 2 {
                return None;
            }
            let sum: f32 = sequence
                .iter()
                .map(|part| f32::from(part.full_advance) / self.upem)
                .sum();
            let joints = (sequence.len() - 1) as f32;
            if sum - min_overlap * joints < target {
                continue;
            }
            let max_overlap = sequence
                .windows(2)
                .map(|pair| {
                    f32::from(
                        pair[0]
                            .end_connector_length
                            .min(pair[1].start_connector_length),
                    ) / self.upem
                })
                .fold(f32::INFINITY, f32::min);
            let overlap =
                ((sum - target) / joints).clamp(min_overlap, max_overlap.max(min_overlap));

            let mut placed = Vec::with_capacity(sequence.len());
            let mut bottom = 0.0;
            for part in &sequence {
                placed.push((GlyphId(part.glyph_id.0), bottom));
                bottom += f32::from(part.full_advance) / self.upem - overlap;
            }
            return Some(Stretched::Assembly {
                parts: placed,
                extent: bottom + overlap,
            });
        }
        None
    }
}

impl GlyphOutline for Face<'_> {
    fn outline(&self, glyph: GlyphId, sink: &mut dyn PathSink) -> bool {
        let mut builder = EmOutline {
            sink,
            upem: self.upem,
        };
        self.face
            .outline_glyph(SfntGlyphId(glyph.0), &mut builder)
            .is_some()
    }
}

/// Converts ttf-parser's design-unit outline callbacks to em.
struct EmOutline<'a> {
    sink: &'a mut dyn PathSink,
    upem: f32,
}

impl OutlineBuilder for EmOutline<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.sink.move_to(x / self.upem, y / self.upem);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.sink.line_to(x / self.upem, y / self.upem);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.sink
            .quad_to(x1 / self.upem, y1 / self.upem, x / self.upem, y / self.upem);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.sink.curve_to(
            x1 / self.upem,
            y1 / self.upem,
            x2 / self.upem,
            y2 / self.upem,
            x / self.upem,
            y / self.upem,
        );
    }

    fn close(&mut self) {
        self.sink.close();
    }
}

fn read_constants(face: &SfntFace<'_>, upem: f32) -> Constants {
    let table = face.tables().math.and_then(|tables| tables.constants);
    let percent = |value: Option<i16>, fallback: f32| {
        value
            .map(|value| f32::from(value) / 100.0)
            .filter(|value| *value > 0.0)
            .unwrap_or(fallback)
    };
    let script = percent(
        table.map(|constants| constants.script_percent_scale_down()),
        0.71,
    );
    let script_script = percent(
        table.map(|constants| constants.script_script_percent_scale_down()),
        script * script,
    );
    let metric = |field: Option<MathValue<'_>>, fallback: f32| em_value(field, upem, fallback);
    Constants {
        script_percent_scale_down: script,
        script_script_percent_scale_down: script_script,
        axis_height: metric(table.map(|c| c.axis_height()), 0.25),
        fraction_rule_thickness: metric(table.map(|c| c.fraction_rule_thickness()), 0.04),
        radical_rule_thickness: metric(table.map(|c| c.radical_rule_thickness()), 0.04),
        space_after_script: metric(table.map(|c| c.space_after_script()), 0.05),
        superscript_shift_up: metric(table.map(|c| c.superscript_shift_up()), 0.413),
        superscript_shift_up_cramped: metric(
            table.map(|c| c.superscript_shift_up_cramped()),
            0.289,
        ),
        superscript_baseline_drop_max: metric(
            table.map(|c| c.superscript_baseline_drop_max()),
            0.386,
        ),
        subscript_shift_down: metric(table.map(|c| c.subscript_shift_down()), 0.15),
        subscript_baseline_drop_min: metric(table.map(|c| c.subscript_baseline_drop_min()), 0.05),
        sub_superscript_gap_min: metric(table.map(|c| c.sub_superscript_gap_min()), 0.247),
        fraction_numerator_shift_up: metric(table.map(|c| c.fraction_numerator_shift_up()), 0.394),
        fraction_denominator_shift_down: metric(
            table.map(|c| c.fraction_denominator_shift_down()),
            0.345,
        ),
        fraction_numerator_display_shift_up: metric(
            table.map(|c| c.fraction_numerator_display_style_shift_up()),
            0.677,
        ),
        fraction_denominator_display_shift_down: metric(
            table.map(|c| c.fraction_denominator_display_style_shift_down()),
            0.686,
        ),
        fraction_numerator_gap_min: metric(table.map(|c| c.fraction_numerator_gap_min()), 0.04),
        fraction_denominator_gap_min: metric(table.map(|c| c.fraction_denominator_gap_min()), 0.04),
        fraction_numerator_display_gap_min: metric(
            table.map(|c| c.fraction_num_display_style_gap_min()),
            0.12,
        ),
        fraction_denominator_display_gap_min: metric(
            table.map(|c| c.fraction_denom_display_style_gap_min()),
            0.12,
        ),
        stack_top_shift_up: metric(table.map(|c| c.stack_top_shift_up()), 0.3),
        stack_bottom_shift_down: metric(table.map(|c| c.stack_bottom_shift_down()), 0.3),
        stack_gap_min: metric(table.map(|c| c.stack_gap_min()), 0.12),
        stack_top_display_shift_up: metric(
            table.map(|c| c.stack_top_display_style_shift_up()),
            0.4,
        ),
        stack_bottom_display_shift_down: metric(
            table.map(|c| c.stack_bottom_display_style_shift_down()),
            0.4,
        ),
        stack_display_gap_min: metric(table.map(|c| c.stack_display_style_gap_min()), 0.2),
        radical_vertical_gap: metric(table.map(|c| c.radical_vertical_gap()), 0.05),
        radical_display_vertical_gap: metric(
            table.map(|c| c.radical_display_style_vertical_gap()),
            0.15,
        ),
        radical_extra_ascender: metric(table.map(|c| c.radical_extra_ascender()), 0.04),
        radical_kern_before_degree: metric(table.map(|c| c.radical_kern_before_degree()), 0.0),
        radical_kern_after_degree: metric(table.map(|c| c.radical_kern_after_degree()), 0.1),
        radical_degree_bottom_raise: percent(
            table.map(|c| c.radical_degree_bottom_raise_percent()),
            0.6,
        ),
        accent_base_height: metric(table.map(|c| c.accent_base_height()), 0.5),
        overbar_vertical_gap: metric(table.map(|c| c.overbar_vertical_gap()), 0.1),
        overbar_rule_thickness: metric(table.map(|c| c.overbar_rule_thickness()), 0.04),
        overbar_extra_ascender: metric(table.map(|c| c.overbar_extra_ascender()), 0.1),
        underbar_vertical_gap: metric(table.map(|c| c.underbar_vertical_gap()), 0.1),
        underbar_rule_thickness: metric(table.map(|c| c.underbar_rule_thickness()), 0.04),
        underbar_extra_descender: metric(table.map(|c| c.underbar_extra_descender()), 0.1),
        display_operator_min_height: table
            .map_or(1.5, |c| f32::from(c.display_operator_min_height()) / upem),
        upper_limit_gap_min: metric(table.map(|c| c.upper_limit_gap_min()), 0.1),
        upper_limit_baseline_rise_min: metric(
            table.map(|c| c.upper_limit_baseline_rise_min()),
            0.2,
        ),
        lower_limit_gap_min: metric(table.map(|c| c.lower_limit_gap_min()), 0.1),
        lower_limit_baseline_drop_min: metric(
            table.map(|c| c.lower_limit_baseline_drop_min()),
            0.2,
        ),
    }
}

fn em_value(value: Option<MathValue<'_>>, upem: f32, fallback: f32) -> f32 {
    value.map_or(fallback, |value| f32::from(value.value) / upem)
}

/// Collects the `ssty` alternate subtables from GSUB.
///
/// Math fonts register `ssty` under the `math` script; reading the feature
/// list directly avoids a shaping pass for single-glyph math atoms.
fn read_ssty<'a>(face: &SfntFace<'a>) -> Vec<AlternateSubstitution<'a>> {
    let mut out = Vec::new();
    let Some(gsub) = face.tables().gsub else {
        return out;
    };
    for feature in gsub.features {
        if feature.tag != Tag::from_bytes(b"ssty") {
            continue;
        }
        for index in feature.lookup_indices {
            let Some(lookup) = gsub.lookups.get(index) else {
                continue;
            };
            for subtable in lookup.subtables.into_iter::<SubstitutionSubtable>() {
                if let SubstitutionSubtable::Alternate(alternate) = subtable {
                    out.push(alternate);
                }
            }
        }
    }
    out
}
