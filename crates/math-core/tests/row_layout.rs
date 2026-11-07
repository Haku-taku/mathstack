use math_core::{
    Alignment, Atom, AtomClass, Color, ColumnSpacing, Constants, DelimiterSize, FractionRule, Gap,
    GlyphId, InkBounds, Item, Layout, Length, LengthUnit, Limits, MathFont, Model, ModelBuilder,
    NodeId, NodeKind, PhantomKind, ShapedGlyph, ShapedRun, Size, Span, Stretched, Style,
    TextShaper, TextStyle, Variant, gap, layout, layout_with_shaper, math_italic, variant_char,
};

struct FakeFont;

impl MathFont for FakeFont {
    fn glyph(&self, _symbol: char) -> Option<GlyphId> {
        Some(GlyphId(1))
    }

    fn advance(&self, _glyph: GlyphId) -> f32 {
        0.5
    }

    fn ink(&self, _glyph: GlyphId) -> Option<InkBounds> {
        Some(InkBounds {
            left: 0.0,
            bottom: -0.2,
            right: 0.4,
            top: 0.6,
        })
    }

    fn constants(&self) -> Constants {
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

/// A face with script alternates at +100 per level.
struct Alternates;

impl MathFont for Alternates {
    fn glyph(&self, symbol: char) -> Option<GlyphId> {
        FakeFont.glyph(symbol)
    }

    fn advance(&self, glyph: GlyphId) -> f32 {
        FakeFont.advance(glyph)
    }

    fn ink(&self, glyph: GlyphId) -> Option<InkBounds> {
        FakeFont.ink(glyph)
    }

    fn constants(&self) -> Constants {
        FakeFont.constants()
    }

    fn script_alternate(&self, glyph: GlyphId, level: u8) -> Option<GlyphId> {
        Some(GlyphId(glyph.0 + u16::from(level) * 100))
    }
}

/// A face whose glyphs grow to any requested height, like an assembly.
struct Growable;

impl MathFont for Growable {
    fn glyph(&self, symbol: char) -> Option<GlyphId> {
        FakeFont.glyph(symbol)
    }

    fn advance(&self, glyph: GlyphId) -> f32 {
        FakeFont.advance(glyph)
    }

    fn ink(&self, glyph: GlyphId) -> Option<InkBounds> {
        FakeFont.ink(glyph)
    }

    fn constants(&self) -> Constants {
        FakeFont.constants()
    }

    fn stretch_vertical(&self, glyph: GlyphId, target: f32) -> Stretched {
        if target > 0.8 {
            Stretched::Assembly {
                parts: vec![(glyph, 0.0)],
                extent: target,
            }
        } else {
            Stretched::Glyph(glyph)
        }
    }

    fn stretch_horizontal(&self, glyph: GlyphId, target: f32) -> Stretched {
        if target > 0.8 {
            // Two parts, the second at the far end, so repetition is visible.
            Stretched::Assembly {
                parts: vec![(glyph, 0.0), (glyph, (target - 0.5).max(0.0))],
                extent: target,
            }
        } else {
            Stretched::Glyph(glyph)
        }
    }
}

/// A face whose glyphs lean right by 0.2 em.
struct Slanted;

impl MathFont for Slanted {
    fn glyph(&self, symbol: char) -> Option<GlyphId> {
        FakeFont.glyph(symbol)
    }

    fn advance(&self, glyph: GlyphId) -> f32 {
        FakeFont.advance(glyph)
    }

    fn ink(&self, glyph: GlyphId) -> Option<InkBounds> {
        FakeFont.ink(glyph)
    }

    fn constants(&self) -> Constants {
        FakeFont.constants()
    }

    fn italic(&self, _glyph: GlyphId) -> f32 {
        0.2
    }
}

/// A model whose root is one row of `text`'s characters.
fn row_model(text: &str) -> Model {
    Model::build(|model| {
        let children = text.chars().map(|symbol| model.symbol(symbol)).collect();
        model.row(children)
    })
    .0
}

fn measured(model: &Model, style: Style) -> Layout {
    layout(&FakeFont, model, model.root(), 10.0, style).0
}

fn width(model: &Model, style: Style) -> f32 {
    measured(model, style).width
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 1e-4
}

fn glyphs(layout: &Layout) -> Vec<(f32, f32)> {
    layout
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Glyph(glyph) => Some((glyph.x, glyph.baseline)),
            Item::Background(_) | Item::Rule(_) | Item::Line(_) => None,
        })
        .collect()
}

#[test]
fn spacing_table_matches_tex_pairs() {
    assert_eq!(gap(AtomClass::Ord, AtomClass::Bin, false), Gap::Medium);
    assert_eq!(gap(AtomClass::Ord, AtomClass::Rel, false), Gap::Thick);
    assert_eq!(gap(AtomClass::Ord, AtomClass::Op, false), Gap::Thin);
    assert_eq!(gap(AtomClass::Open, AtomClass::Ord, false), Gap::None);
    assert_eq!(gap(AtomClass::Ord, AtomClass::Bin, true), Gap::None);
    assert_eq!(gap(AtomClass::Ord, AtomClass::Op, true), Gap::Thin);
    assert_eq!(gap(AtomClass::Inner, AtomClass::Ord, true), Gap::Thin);
    assert!(close(Gap::Thin.em(), 3.0 / 18.0));
}

#[test]
fn binary_operators_cancel_at_row_edges() {
    let medium = 4.0 / 18.0 * 10.0;
    assert!(close(
        width(&row_model("a+b"), Style::TEXT),
        15.0 + 2.0 * medium
    ));

    // A leading or trailing Bin becomes Ord, so it carries no glue.
    assert!(close(width(&row_model("+a"), Style::TEXT), 10.0));
    assert!(close(width(&row_model("a+"), Style::TEXT), 10.0));

    // `a+=b`: the plus is followed by Rel and cancels; two thick gaps remain.
    let thick = 5.0 / 18.0 * 10.0;
    assert!(close(
        width(&row_model("a+=b"), Style::TEXT),
        20.0 + 2.0 * thick
    ));
}

#[test]
fn script_styles_drop_medium_and_thick_glue() {
    let em = 10.0 * 0.7;
    assert!(close(
        width(&row_model("a+b"), Style::SCRIPT),
        3.0 * 0.5 * em
    ));
    assert!(close(
        width(&row_model("a=b"), Style::SCRIPT),
        3.0 * 0.5 * em
    ));

    // Ord–Op thin survives the tight table.
    assert!(close(
        width(&row_model("a∑"), Style::SCRIPT),
        2.0 * 0.5 * em + 3.0 / 18.0 * em
    ));
}

#[test]
fn style_transitions_follow_the_tex_table() {
    assert_eq!(Style::DISPLAY.sup(), Style::SCRIPT);
    assert_eq!(Style::DISPLAY.sub(), Style::SCRIPT_CRAMPED);
    assert_eq!(Style::TEXT_CRAMPED.sup(), Style::SCRIPT_CRAMPED);
    assert_eq!(Style::SCRIPT.sup(), Style::SCRIPT_SCRIPT);
    assert_eq!(Style::SCRIPT.sub(), Style::SCRIPT_SCRIPT_CRAMPED);
    assert_eq!(Style::DISPLAY.frac_num(), Style::TEXT);
    assert_eq!(Style::DISPLAY.frac_den(), Style::TEXT_CRAMPED);
    assert_eq!(Style::TEXT.frac_num(), Style::SCRIPT);
    assert_eq!(Style::TEXT.frac_den(), Style::SCRIPT_CRAMPED);
    assert_eq!(
        Style::SCRIPT_SCRIPT.frac_den(),
        Style::SCRIPT_SCRIPT_CRAMPED
    );
    assert_eq!(Style::TEXT.cramped().size, Size::Text);
    assert!(Style::TEXT.cramped().cramped);
}

#[test]
fn superscript_geometry_follows_rule_18c() {
    let model = Model::build(|m| {
        let x = m.symbol('x');
        let two = m.symbol('2');
        m.scripts(x, Some(two), None)
    })
    .0;
    let layout = measured(&model, Style::TEXT);
    // x is 5 wide; the script sets at 7pt and is raised to 4pt.
    assert!(close(layout.width, 5.0 + 3.5 + 0.5));
    assert!(close(layout.ascent, 4.0 + 4.2));
    assert!(close(layout.descent, 2.0));
    let glyphs = glyphs(&layout);
    assert!(close(glyphs[1].0, 5.0));
    assert!(close(glyphs[1].1, 8.2 - 4.0));
}

#[test]
fn both_scripts_keep_four_rule_widths_of_clearance() {
    let model = Model::build(|m| {
        let x = m.symbol('x');
        let two = m.symbol('2');
        let one = m.symbol('1');
        m.scripts(x, Some(two), Some(one))
    })
    .0;
    let layout = measured(&model, Style::TEXT);
    // Rule 18e raises the superscript to 5.4 and settles the subscript at 1.8.
    assert!(close(layout.ascent, 5.4 + 4.2));
    assert!(close(layout.descent, 1.8 + 1.4));
    assert!(close(layout.width, 5.0 + 3.5 + 0.5));
}

#[test]
fn italic_correction_moves_the_superscript_only() {
    let (model, root) = Model::build(|m| {
        let f = m.symbol('f');
        let two = m.symbol('2');
        let one = m.symbol('1');
        m.scripts(f, Some(two), Some(one))
    });
    let layout = layout(&Slanted, &model, root, 10.0, Style::TEXT).0;
    let glyphs = glyphs(&layout);
    // The superscript starts after the 0.2em italic correction, the subscript
    // stays at the advance (Rule 18f), so the width grows to the superscript.
    assert!(close(glyphs[1].0, 5.0 + 2.0));
    assert!(close(glyphs[2].0, 5.0));
    assert!(close(layout.width, 7.0 + 3.5 + 0.5));
}

#[test]
fn fraction_geometry_uses_math_gaps_and_axis() {
    let model = Model::build(|m| {
        let a = m.symbol('a');
        let b = m.symbol('b');
        m.fraction(a, b, true)
    })
    .0;
    let layout = measured(&model, Style::TEXT);
    // Numerator clears the rule by its gap, so it sits 5.1 high, not 4.0.
    assert!(close(layout.width, 3.5));
    assert!(close(layout.ascent, 5.1 + 4.2));
    assert!(close(layout.descent, 3.0 + 1.4));
    let rule = layout
        .items
        .iter()
        .find_map(|item| match item {
            Item::Rule(rule) => Some(rule),
            Item::Background(_) | Item::Line(_) | Item::Glyph(_) => None,
        })
        .expect("a bar fraction has a rule");
    assert!(close(rule.y, 9.3 - 2.7));
    assert!(close(rule.width, 3.5));
    assert!(close(rule.height, 0.4));
}

#[test]
fn barless_fraction_uses_the_stack_constants() {
    let model = Model::build(|m| {
        let a = m.symbol('a');
        let b = m.symbol('b');
        m.fraction(a, b, false)
    })
    .0;
    let layout = measured(&model, Style::TEXT);
    // The preferred 3.0 shifts leave only 0.4 of clearance, so both parts move
    // out by 0.4 to reach the 1.2 stack gap.
    assert!(close(layout.ascent, 3.4 + 4.2));
    assert!(close(layout.descent, 3.4 + 1.4));
    assert!(close(layout.width, 3.5));
    assert!(
        !layout
            .items
            .iter()
            .any(|item| matches!(item, Item::Rule(_)))
    );
}

#[test]
fn radical_with_degree_places_the_index_and_overbar() {
    let model = Model::build(|m| {
        let a = m.symbol('a');
        let three = m.symbol('3');
        m.radical(a, Some(three))
    })
    .0;
    let layout = measured(&model, Style::TEXT);
    // The surd spans 8pt: 1pt gap and 0.4pt rule sit above the 6pt radicand.
    // The degree sets at 4.9pt and is raised 60% of the surd height.
    assert!(close(layout.width, 0.5 + 2.45 + 1.0 + 5.0 + 5.0));
    assert!(close(layout.ascent, 8.12));
    assert!(close(layout.descent, 2.0));
    let rule = layout
        .items
        .iter()
        .find_map(|item| match item {
            Item::Rule(rule) => Some(rule),
            Item::Background(_) | Item::Line(_) | Item::Glyph(_) => None,
        })
        .expect("a radical has an overbar");
    assert!(close(rule.x, 3.95 + 5.0));
    assert!(close(rule.y, 8.12 - 7.4));
    assert!(close(rule.width, 5.0));
}

#[test]
fn script_levels_take_the_fonts_alternates() {
    let (model, root) = Model::build(|m| {
        let x = m.symbol('x');
        let two = m.symbol('2');
        let three = m.symbol('3');
        let inner = m.scripts(two, Some(three), None);
        m.scripts(x, Some(inner), None)
    });
    let layout = layout(&Alternates, &model, root, 10.0, Style::TEXT).0;
    let ids: Vec<u16> = layout
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Glyph(glyph) => Some(glyph.glyph.0),
            Item::Background(_) | Item::Rule(_) | Item::Line(_) => None,
        })
        .collect();
    // The base keeps its glyph; the two script depths take their alternates
    // (the fake maps level 1 to +100 and level 2 to +200).
    assert_eq!(ids, vec![1, 101, 201]);
}

#[test]
fn delimiters_grow_symmetrically_about_the_axis() {
    let model = Model::build(|m| {
        let a = m.symbol('a');
        m.delimited(Some('('), a, Some(')'))
    })
    .0;
    let layout = measured(&model, Style::TEXT);
    // The body reaches 4.5 below the axis, so each fence targets 9pt; the 8pt
    // glyph is centred on the axis and the body keeps its baseline.
    assert!(close(layout.width, 15.0));
    assert!(close(layout.ascent, 7.0));
    assert!(close(layout.descent, 2.0));

    let model = Model::build(|m| {
        let a = m.symbol('a');
        m.delimited(None, a, Some(')'))
    })
    .0;
    let layout = measured(&model, Style::TEXT);
    assert!(close(layout.width, 10.0));
    assert!(close(glyphs(&layout)[0].0, 0.0));
}

#[test]
fn large_operator_grows_and_centres_on_the_axis_in_display() {
    let (model, root) = Model::build(|m| m.large_op('∑', false));
    let display = layout(&Growable, &model, root, 10.0, Style::DISPLAY).0;
    assert!(close(display.width, 5.0));
    assert!(close(display.ascent, 2.5 + 12.0 / 2.0));
    assert!(close(display.descent, 12.0 - display.ascent));

    let text = layout(&Growable, &model, root, 10.0, Style::TEXT).0;
    assert!(close(text.ascent, 2.5 + 8.0 / 2.0));
    assert!(close(text.descent, 8.0 - text.ascent));
}

#[test]
fn limits_are_display_only_and_clear_the_operator() {
    let (model, root) = Model::build(|m| {
        let sum = m.large_op('∑', true);
        let n = m.symbol('n');
        let i = m.symbol('i');
        m.scripts(sum, Some(n), Some(i))
    });

    let display = layout(&Growable, &model, root, 10.0, Style::DISPLAY).0;
    // Upper limit clears the 8.5pt operator by 2.4; lower by 5.2.
    assert!(close(display.width, 5.0));
    assert!(close(display.ascent, 10.9 + 4.2));
    assert!(close(display.descent, 8.7 + 1.4));

    let text = layout(&Growable, &model, root, 10.0, Style::TEXT).0;
    // Outside display style the same scripts sit at the side (Rule 18).
    assert!(close(text.ascent, 5.4 + 4.2));
    assert!(close(text.descent, 1.8 + 1.4));
}

#[test]
fn operator_suffixes_override_the_display_default() {
    // `\sum\limits` in text style: limits above and below, centred.
    let (model, root) = Model::build(|m| {
        let sum = m.large_op('∑', true);
        m.replace(
            sum,
            NodeKind::LargeOp {
                symbol: '∑',
                limits: Limits::AboveBelow,
            },
        );
        let n = m.symbol('n');
        let i = m.symbol('i');
        m.scripts(sum, Some(n), Some(i))
    });
    let text = layout(&Growable, &model, root, 10.0, Style::TEXT).0;
    assert!(close(text.ascent, 8.9 + 4.2));
    assert!(close(text.descent, 6.7 + 1.4));
    assert!(close(text.width, 5.0));

    // `\sum\nolimits` in display style: side scripts, like an integral's.
    let (model, root) = Model::build(|m| {
        let sum = m.large_op('∑', true);
        m.replace(
            sum,
            NodeKind::LargeOp {
                symbol: '∑',
                limits: Limits::Beside,
            },
        );
        let n = m.symbol('n');
        let i = m.symbol('i');
        m.scripts(sum, Some(n), Some(i))
    });
    let display = layout(&Growable, &model, root, 10.0, Style::DISPLAY).0;
    assert!(close(display.ascent, 9.7));
    assert!(close(display.descent, 5.9));
    assert!(close(display.width, 9.0));
}

#[test]
fn large_operator_scripts_follow_its_height_and_depth() {
    let (model, root) = Model::build(|m| {
        let integral = m.large_op('∫', false);
        let one = m.symbol('1');
        let zero = m.symbol('0');
        m.scripts(integral, Some(one), Some(zero))
    });

    // A wide operator is not a character box: the 12pt display glyph raises the
    // superscript by 5.5 and drops the subscript by 4.5, not by the minima.
    let display = layout(&Growable, &model, root, 10.0, Style::DISPLAY).0;
    let glyphs = glyphs(&display);
    assert!(close(display.ascent, 9.7));
    assert!(close(display.descent, 5.9));
    assert!(close(glyphs[1].1, 9.7 - 5.5));
    assert!(close(glyphs[2].1, 9.7 + 4.5));
}

#[test]
fn single_element_group_scripts_like_its_base() {
    // `{f}_1`: the subscript hangs from f's depth, not the group's baseline drop.
    let (model, root) = Model::build(|m| {
        let f = m.symbol('f');
        let group = m.row(vec![f]);
        let one = m.symbol('1');
        m.scripts(group, None, Some(one))
    });
    let subscript = layout(&Slanted, &model, root, 10.0, Style::TEXT).0;
    assert!(close(subscript.descent, 2.0 + 1.4));

    // `{f}^2` keeps f's italic correction for the superscript.
    let (model, root) = Model::build(|m| {
        let f = m.symbol('f');
        let group = m.row(vec![f]);
        let two = m.symbol('2');
        m.scripts(group, Some(two), None)
    });
    let superscript = layout(&Slanted, &model, root, 10.0, Style::TEXT).0;
    assert!(close(glyphs(&superscript)[1].0, 5.0 + 2.0));
}

#[test]
fn latin_letters_take_the_math_italic_codepoint() {
    assert_eq!(math_italic('a'), Some('\u{1D44E}'));
    assert_eq!(math_italic('H'), Some('\u{1D43B}'));
    assert_eq!(math_italic('B'), Some('\u{1D435}'));
    // The italic small-h slot is a hole; unicode-math uses the Planck constant.
    assert_eq!(math_italic('h'), Some('\u{210E}'));
    assert_eq!(math_italic('1'), None);
}

#[test]
fn row_metrics_come_from_glyph_ink() {
    let layout = measured(&row_model("ab"), Style::TEXT);
    assert!(close(layout.ascent, 6.0));
    assert!(close(layout.descent, 2.0));
    let glyphs = glyphs(&layout);
    assert!(close(glyphs[0].1, 6.0));
}

/// A face with one glyph id per interesting codepoint, so a test can see which
/// codepoint layout chose.
struct Coded;

impl MathFont for Coded {
    fn glyph(&self, symbol: char) -> Option<GlyphId> {
        let id = match symbol {
            'x' => 10,
            '\u{1D465}' => 11,
            'A' => 3,
            '\u{1D400}' => 4,
            '\u{211D}' => 12,
            '\u{02C6}' => 20,
            other => u16::try_from(other as u32).ok()?,
        };
        Some(GlyphId(id))
    }

    fn advance(&self, _glyph: GlyphId) -> f32 {
        0.5
    }

    fn ink(&self, _glyph: GlyphId) -> Option<InkBounds> {
        Some(InkBounds {
            left: 0.0,
            bottom: -0.2,
            right: 0.4,
            top: 0.6,
        })
    }

    fn constants(&self) -> Constants {
        FakeFont.constants()
    }

    fn top_accent_attachment(&self, glyph: GlyphId) -> Option<f32> {
        match glyph.0 {
            10 | 11 => Some(0.2),
            20 => Some(0.6),
            _ => None,
        }
    }
}

fn glyph_ids(layout: &Layout) -> Vec<u16> {
    layout
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Glyph(glyph) => Some(glyph.glyph.0),
            Item::Background(_) | Item::Rule(_) | Item::Line(_) => None,
        })
        .collect()
}

fn coded_ids(build: impl FnOnce(&mut ModelBuilder) -> NodeId) -> Vec<u16> {
    let (model, root) = Model::build(build);
    glyph_ids(&layout(&Coded, &model, root, 10.0, Style::TEXT).0)
}

#[test]
fn variants_resolve_to_their_unicode_block() {
    assert_eq!(variant_char('R', Variant::DoubleStruck), Some('\u{211D}'));
    assert_eq!(variant_char('C', Variant::DoubleStruck), Some('\u{2102}'));
    assert_eq!(variant_char('B', Variant::Script), Some('\u{212C}'));
    assert_eq!(variant_char('e', Variant::Script), Some('\u{212F}'));
    assert_eq!(variant_char('h', Variant::Italic), Some('\u{210E}'));
    assert_eq!(variant_char('7', Variant::Bold), Some('\u{1D7D5}'));
    assert_eq!(variant_char('α', Variant::Bold), None);

    // Ordinary letters take the math italic block, variants their own block.
    assert_eq!(coded_ids(|m| m.symbol('x')), vec![11]);
    assert_eq!(
        coded_ids(|m| m.atom(Atom::with_variant('A', Variant::Bold))),
        vec![4]
    );
    assert_eq!(
        coded_ids(|m| m.atom(Atom::with_variant('R', Variant::DoubleStruck))),
        vec![12]
    );
    // A font-command container supplies the variant of its whole body, while
    // an atom that already names one keeps it.
    assert_eq!(
        coded_ids(|m| {
            let r = m.symbol('R');
            m.variant(Variant::DoubleStruck, r)
        }),
        vec![12]
    );
    assert_eq!(
        coded_ids(|m| {
            let a = m.atom(Atom::with_variant('A', Variant::Bold));
            m.variant(Variant::DoubleStruck, a)
        }),
        vec![4]
    );
}

#[test]
fn accents_center_on_the_math_attachment_and_clear_the_base() {
    let (model, root) = Model::build(|m| {
        let x = m.symbol('x');
        m.accent(x, '\u{02C6}')
    });
    let layout = layout(&Coded, &model, root, 10.0, Style::TEXT).0;
    // The base is 6pt tall, so a 0.5em accent base height lifts the mark 1pt;
    // the attachments (0.2em vs 0.6em) pull the mark 4pt left.
    assert!(close(layout.width, 5.0));
    assert!(close(layout.ascent, 7.0));
    assert!(close(layout.descent, 2.0));
    let glyphs: Vec<(u16, f32, f32)> = layout
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Glyph(glyph) => Some((glyph.glyph.0, glyph.x, glyph.baseline)),
            Item::Background(_) | Item::Rule(_) | Item::Line(_) => None,
        })
        .collect();
    assert_eq!(glyphs.len(), 2);
    assert_eq!((glyphs[0].0, glyphs[1].0), (11, 20));
    assert!(close(glyphs[0].1, 0.0));
    assert!(close(glyphs[0].2, 7.0));
    assert!(close(glyphs[1].1, -4.0));
    assert!(close(glyphs[1].2, 6.0));
}

#[test]
fn text_keeps_spaces_and_their_advances() {
    let (model, root) = Model::build(|m| m.text("a b"));
    // Three glyphs at the fake font's 0.5em advance; the space adds no ink.
    let layout = layout(&Coded, &model, root, 10.0, Style::TEXT).0;
    assert!(close(layout.width, 15.0));
    assert!(close(layout.ascent, 6.0));
    assert!(close(layout.descent, 2.0));
}

#[test]
fn spacing_commands_are_glue_not_atoms() {
    // A thin space adds width without changing the classes around it, so the
    // plus keeps its medium glue before it.
    let thin = 3.0 / 18.0 * 10.0;
    assert!(close(width(&row_model("ab"), Style::TEXT), 10.0));
    let model = Model::build(|m| {
        let a = m.symbol('a');
        let space = m.space(math_core::SpaceKind::Thin);
        let b = m.symbol('b');
        m.row(vec![a, space, b])
    })
    .0;
    assert!(close(width(&model, Style::TEXT), 10.0 + thin));
    let model = Model::build(|m| {
        let a = m.symbol('a');
        let space = m.space(math_core::SpaceKind::Thin);
        let plus = m.symbol('+');
        let b = m.symbol('b');
        m.row(vec![a, space, plus, b])
    })
    .0;
    // Binary operators keep their medium glue on both sides.
    let medium = 4.0 / 18.0 * 10.0;
    assert!(close(
        width(&model, Style::TEXT),
        15.0 + thin + 2.0 * medium
    ));
}

#[test]
fn bars_span_the_base_and_clear_it_by_the_math_gaps() {
    let model = Model::build(|m| {
        let a = m.symbol('a');
        m.overline(a)
    })
    .0;
    let over = measured(&model, Style::TEXT);
    assert!(close(over.width, 5.0));
    assert!(close(over.ascent, 8.4));
    assert!(close(over.descent, 2.0));
    let rule = over
        .items
        .iter()
        .find_map(|item| match item {
            Item::Rule(rule) => Some(rule),
            Item::Background(_) | Item::Line(_) | Item::Glyph(_) => None,
        })
        .expect("an overline has a rule");
    assert!(close(rule.y, 1.0));
    assert!(close(rule.height, 0.4));

    let model = Model::build(|m| {
        let a = m.symbol('a');
        m.underline(a)
    })
    .0;
    let under = measured(&model, Style::TEXT);
    assert!(close(under.ascent, 6.0));
    assert!(close(under.descent, 4.4));
    let rule = under
        .items
        .iter()
        .find_map(|item| match item {
            Item::Rule(rule) => Some(rule),
            Item::Background(_) | Item::Line(_) | Item::Glyph(_) => None,
        })
        .expect("an underline has a rule");
    assert!(close(rule.y, 9.0));
}

#[test]
fn sized_delimiters_use_their_fixed_height() {
    let (model, root) =
        Model::build(|m| m.sized_delimiter('(', DelimiterSize::Big, AtomClass::Open));
    let big = layout(&Growable, &model, root, 10.0, Style::TEXT).0;
    // 12pt tall, centred on the 2.5pt axis.
    assert!(close(big.width, 5.0));
    assert!(close(big.ascent, 8.5));
    assert!(close(big.descent, 3.5));

    let (model, root) =
        Model::build(|m| m.sized_delimiter('(', DelimiterSize::BiggerStill, AtomClass::Open));
    let bigger = layout(&Growable, &model, root, 10.0, Style::TEXT).0;
    assert!(close(bigger.ascent, 2.5 + 12.0));
}

#[test]
fn middle_delimiters_grow_with_the_enclosing_group() {
    let model = Model::build(|m| {
        let a = m.symbol('a');
        let middle = m.middle('|');
        let b = m.symbol('b');
        let body = m.row(vec![a, middle, b]);
        m.delimited(Some('('), body, Some(')'))
    })
    .0;
    let with_middle = measured(&model, Style::TEXT);
    // Three 9pt fences at 5pt each plus the two letters.
    assert!(close(with_middle.width, 25.0));
    assert!(close(with_middle.ascent, 7.0));

    let model = Model::build(|m| {
        let children = "ab".chars().map(|symbol| m.symbol(symbol)).collect();
        let body = m.row(children);
        m.delimited(Some('('), body, Some(')'))
    })
    .0;
    let plain = measured(&model, Style::TEXT);
    assert!(close(plain.width, 20.0));
}

#[test]
fn phantoms_smash_and_boxes_trade_space_for_ink() {
    let model = Model::build(|m| {
        let a = m.symbol('a');
        m.phantom(a, PhantomKind::Full)
    })
    .0;
    let phantom = measured(&model, Style::TEXT);
    assert!(close(phantom.width, 5.0));
    assert!(close(phantom.ascent, 6.0));
    assert!(close(phantom.descent, 2.0));
    assert!(phantom.items.is_empty());

    let model = Model::build(|m| {
        let a = m.symbol('a');
        m.phantom(a, PhantomKind::Horizontal)
    })
    .0;
    let horizontal = measured(&model, Style::TEXT);
    assert!(close(horizontal.width, 5.0));
    assert!(close(horizontal.ascent, 0.0));

    let model = Model::build(|m| {
        let a = m.symbol('a');
        m.phantom(a, PhantomKind::Vertical)
    })
    .0;
    let vertical = measured(&model, Style::TEXT);
    assert!(close(vertical.width, 0.0));
    assert!(close(vertical.ascent, 6.0));

    let model = Model::build(|m| {
        let a = m.symbol('a');
        m.smash(a, true, false)
    })
    .0;
    let smashed = measured(&model, Style::TEXT);
    assert!(close(smashed.width, 5.0));
    assert!(close(smashed.ascent, 0.0));
    assert!(close(smashed.descent, 2.0));
    assert_eq!(smashed.items.len(), 1);

    let model = Model::build(|m| {
        let a = m.symbol('a');
        m.boxed(a)
    })
    .0;
    let boxed = measured(&model, Style::TEXT);
    assert!(close(boxed.width, 11.8));
    assert!(close(boxed.ascent, 9.4));
    assert!(close(boxed.descent, 5.4));
    assert_eq!(
        boxed
            .items
            .iter()
            .filter(|item| matches!(item, Item::Rule(_)))
            .count(),
        4
    );
}

#[test]
fn over_and_under_scripts_clear_the_base() {
    let model = Model::build(|m| {
        let equals = m.symbol('=');
        let bang = m.symbol('!');
        m.overset(equals, Some(bang), None)
    })
    .0;
    let over = measured(&model, Style::TEXT);
    // The annotation sets at 7pt, 1.4 below its baseline, and clears the base
    // by the overbar gap plus the extra ascender.
    assert!(close(over.width, 5.0));
    assert!(close(over.ascent, 6.0 + 1.0 + 1.4 + 4.2 + 1.0));
    assert!(close(over.descent, 2.0));
}

#[test]
fn matrix_columns_share_their_widest_cell() {
    let (model, _) = Model::build(|m| {
        let a = m.symbol('a');
        let cell_a = m.row(vec![a]);
        let b = m.symbol('b');
        let b2 = m.symbol('b');
        let cell_bb = m.row(vec![b, b2]);
        let row0 = m.table_row(vec![cell_a, cell_bb]);
        let c = m.symbol('c');
        let cell_c = m.row(vec![c]);
        let d = m.symbol('d');
        let cell_d = m.row(vec![d]);
        let row1 = m.table_row(vec![cell_c, cell_d]);
        m.table(vec![row0, row1], 2, Vec::new())
    });
    let layout = measured(&model, Style::TEXT);
    // Columns are 5 and 10 wide with a 5pt gap; row 1's baseline is 13pt down.
    assert!(close(layout.width, 20.0));
    assert!(close(layout.ascent, 6.0));
    assert!(close(layout.descent, 15.0));
}

#[test]
fn table_alignment_places_narrow_cells_in_their_column() {
    let build = |alignment: Vec<Alignment>| {
        Model::build(|m| {
            let a = m.symbol('a');
            let cell_a = m.row(vec![a]);
            let b = m.symbol('b');
            let b2 = m.symbol('b');
            let cell_bb = m.row(vec![b, b2]);
            let row0 = m.table_row(vec![cell_a, cell_bb]);
            let c = m.symbol('c');
            let cell_c = m.row(vec![c]);
            let d = m.symbol('d');
            let cell_d = m.row(vec![d]);
            let row1 = m.table_row(vec![cell_c, cell_d]);
            m.table(vec![row0, row1], 2, alignment)
        })
        .0
    };
    let left = build(vec![Alignment::Left; 2]);
    let left_xs: Vec<f32> = glyphs(&measured(&left, Style::TEXT))
        .into_iter()
        .map(|(x, _)| x)
        .collect();
    assert_eq!(left_xs, vec![0.0, 10.0, 15.0, 0.0, 10.0]);

    let centred = build(Vec::new());
    let centred_xs: Vec<f32> = glyphs(&measured(&centred, Style::TEXT))
        .into_iter()
        .map(|(x, _)| x)
        .collect();
    // `d` is 5pt wide in a 10pt column, so centring moves it 2.5pt right.
    assert_eq!(centred_xs, vec![0.0, 10.0, 15.0, 0.0, 12.5]);
}

#[test]
fn a_composition_lays_out_like_text() {
    let (composition, _) = Model::build(|m| m.composition("xy"));
    let composed = measured(&composition, Style::TEXT);
    let (text, root) = Model::build(|m| m.text("xy"));
    assert_eq!(root, text.root());
    let shaped = measured(&text, Style::TEXT);
    assert!(close(composed.width, shaped.width));
    assert!(close(composed.ascent, shaped.ascent));
}

#[test]
fn hspace_resolves_em_ex_and_mu_units() {
    let width = |length| {
        let model = Model::build(|m| m.hspace(length)).0;
        self::width(&model, Style::TEXT)
    };
    assert!(close(width(Length::new(1000, LengthUnit::Em)), 10.0));
    // FakeFont's x-height is 0.5, so one ex is half an em.
    assert!(close(width(Length::new(1000, LengthUnit::Ex)), 5.0));
    // Eighteen mu make one em.
    assert!(close(width(Length::new(18000, LengthUnit::Mu)), 10.0));
    assert!(close(width(Length::new(-1000, LengthUnit::Em)), -10.0));
}

#[test]
fn cancel_draws_one_diagonal_per_direction() {
    let lines = |up, down| {
        let model = Model::build(|m| {
            let body = m.symbol('x');
            m.cancel(body, up, down)
        })
        .0;
        let layout = measured(&model, Style::TEXT);
        layout
            .items
            .into_iter()
            .filter_map(|item| match item {
                Item::Line(line) => Some(line),
                _ => None,
            })
            .collect::<Vec<_>>()
    };

    // The body is 5 wide with ink 6 above and 2 below the baseline, so the
    // formula's baseline sits 6 down and the line spans (0, 8) to (5, 0).
    let up = lines(true, false);
    assert_eq!(up.len(), 1);
    assert!(close(up[0].x1, 0.0) && close(up[0].y1, 8.0));
    assert!(close(up[0].x2, 5.0) && close(up[0].y2, 0.0));

    let down = lines(false, true);
    assert_eq!(down.len(), 1);
    assert!(close(down[0].y1, 0.0) && close(down[0].y2, 8.0));

    assert_eq!(lines(true, true).len(), 2);
}

#[test]
fn xrightarrow_stretches_to_its_label() {
    let model = Model::build(|m| {
        let mut label = Vec::new();
        for _ in 0..6 {
            label.push(m.symbol('f'));
        }
        let over = m.row(label);
        m.arrow('\u{2192}', Some(over), None)
    })
    .0;
    let layout = layout(&Growable, &model, model.root(), 10.0, Style::TEXT).0;

    // The label is 21 wide at script size, so the arrow is 26 wide and its two
    // assembly parts sit at the ends.
    assert!(close(layout.width, 26.0));
    let arrow: Vec<f32> = layout
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Glyph(glyph) if close(glyph.x, 0.0) || close(glyph.x, 21.0) => Some(glyph.x),
            _ => None,
        })
        .collect();
    assert_eq!(arrow, vec![0.0, 21.0]);
    assert!(layout.ascent > 6.0, "the label clears the arrow");
}

#[test]
fn braces_sit_on_the_body_and_keep_their_scripts_outside() {
    let model = Model::build(|m| {
        let a = m.symbol('a');
        let b = m.symbol('b');
        let body = m.row(vec![a, b]);
        let brace = m.horizontal_brace(body, true);
        let n = m.symbol('n');
        let superscript = m.row(vec![n]);
        m.scripts(brace, Some(superscript), None)
    })
    .0;
    let layout = measured(&model, Style::TEXT);
    let placed = glyphs(&layout);
    let baselines: Vec<f32> = placed.iter().map(|(_, y)| *y).collect();
    assert_eq!(baselines.len(), 4);
    // Body, then the brace above it, then the superscript above the brace.
    assert!(close(baselines[0], baselines[1]));
    assert!(baselines[2] < baselines[0]);
    assert!(baselines[3] < baselines[2]);
    // Limits, not a side script: the superscript stays over the 10pt brace.
    assert!(placed[3].0 < 10.0);

    let under = Model::build(|m| {
        let a = m.symbol('a');
        let body = m.row(vec![a]);
        m.horizontal_brace(body, false)
    })
    .0;
    let layout = measured(&under, Style::TEXT);
    let baselines: Vec<f32> = glyphs(&layout).into_iter().map(|(_, y)| y).collect();
    assert_eq!(baselines.len(), 2);
    assert!(
        baselines[1] > baselines[0],
        "the underbrace is below the body"
    );
}

#[test]
fn genfrac_uses_its_rule_thickness_and_style() {
    let build = |rule, style| {
        Model::build(|m| {
            let one = m.symbol('1');
            let two = m.symbol('2');
            m.fraction_styled(one, two, rule, style)
        })
        .0
    };

    let custom = build(
        FractionRule::Custom(Length::new(1000, LengthUnit::Em)),
        None,
    );
    let layout = measured(&custom, Style::TEXT);
    let rules: Vec<_> = layout
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Rule(rule) => Some(*rule),
            _ => None,
        })
        .collect();
    assert_eq!(rules.len(), 1);
    assert!(close(rules[0].height, 10.0));

    // A zero rule and a script-level style: no bar, and the parts shrink a
    // second level, to scriptscript.
    let bare = build(FractionRule::None, Some(Size::Script));
    let layout = measured(&bare, Style::TEXT);
    assert!(
        !layout
            .items
            .iter()
            .any(|item| matches!(item, Item::Rule(_)))
    );
    assert!(close(layout.width, 2.45));
}

#[test]
fn forced_fraction_styles_change_the_child_size() {
    let model = Model::build(|m| {
        let one = m.symbol('1');
        let two = m.symbol('2');
        m.fraction(one, two, true)
    })
    .0;
    let frac = measured(&model, Style::TEXT);
    let model = Model::build(|m| {
        let one = m.symbol('1');
        let two = m.symbol('2');
        m.fraction_styled(one, two, FractionRule::Default, Some(Size::Display))
    })
    .0;
    let dfrac = measured(&model, Style::TEXT);
    // The numerator is Script size in a text-style fraction and Text size in
    // a display-style one.
    assert!(close(frac.width, 3.5));
    assert!(close(dfrac.width, 5.0));
    assert!(dfrac.ascent > frac.ascent);
}

/// Claims a fixed run regardless of the text, so a test can tell the host
/// shaper apart from the face fallback.
struct WideShaper;

impl TextShaper for WideShaper {
    fn shape(&self, _text: &str, _style: TextStyle) -> ShapedRun {
        ShapedRun {
            glyphs: vec![ShapedGlyph {
                glyph: GlyphId(9),
                x: 1.0,
                y: 0.5,
                size: 2.0,
            }],
            width: 3.0,
            ascent: 1.0,
            descent: 0.5,
            missing: vec!['z'],
        }
    }
}

#[test]
fn text_layout_uses_the_host_shaper() {
    let (model, root) = Model::build(|m| m.text("ab"));

    let (shaped, diagnostics) =
        layout_with_shaper(&FakeFont, &WideShaper, &model, root, 10.0, Style::TEXT);
    assert_eq!(shaped.width, 30.0);
    assert_eq!(shaped.ascent, 10.0);
    assert_eq!(shaped.descent, 5.0);
    let [Item::Glyph(glyph)] = shaped.items.as_slice() else {
        panic!("the shaper's single glyph is the whole display list");
    };
    assert_eq!(glyph.glyph, GlyphId(9));
    assert_eq!((glyph.x, glyph.baseline, glyph.size), (10.0, 5.0, 20.0));
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("U+007A"));

    let (fallback, _) = layout(&FakeFont, &model, root, 10.0, Style::TEXT);
    assert_eq!(fallback.width, 10.0);
    assert_eq!((fallback.ascent, fallback.descent), (6.0, 2.0));
    assert_eq!(fallback.items.len(), 2);
}

#[test]
fn table_boundaries_carry_rules_and_flush_spacing() {
    let build = |rules: Vec<u8>, spacing| {
        Model::build(|m| {
            let a = m.symbol('a');
            let cell_a = m.row(vec![a]);
            let b = m.symbol('b');
            let cell_b = m.row(vec![b]);
            let row = m.table_row(vec![cell_a, cell_b]);
            m.alloc(
                NodeKind::Table {
                    rows: vec![row],
                    columns: 2,
                    alignment: Vec::new(),
                    cell_style: None,
                    column_rules: rules,
                    column_spacing: spacing,
                },
                math_core::Span::default(),
            )
        })
        .0
    };

    // A rule widens its boundary by its thickness and sits centred in it, so
    // the columns keep their distance; the rule spans the table's height.
    let ruled = measured(&build(vec![1, 1, 1], ColumnSpacing::Spaced), Style::TEXT);
    assert!(close(ruled.width, 26.2));
    let rules: Vec<(f32, f32, f32, f32)> = ruled
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Rule(rule) => Some((rule.x, rule.y, rule.width, rule.height)),
            Item::Background(_) | Item::Glyph(_) | Item::Line(_) => None,
        })
        .collect();
    // The display list is shifted down by the ascent, so the rule starts at 0.
    let expected = [
        (2.5, 0.0, 0.4, 8.0),
        (12.9, 0.0, 0.4, 8.0),
        (23.3, 0.0, 0.4, 8.0),
    ];
    assert_eq!(rules.len(), expected.len());
    for (rule, expected) in rules.iter().zip(expected) {
        assert!(
            close(rule.0, expected.0)
                && close(rule.1, expected.1)
                && close(rule.2, expected.2)
                && close(rule.3, expected.3),
            "rule {rule:?} differs from {expected:?}"
        );
    }

    // `alignat`'s columns are flush: no gap, so the table is the cell widths.
    let flush = measured(&build(Vec::new(), ColumnSpacing::Flush), Style::TEXT);
    assert!(close(flush.width, 10.0));
    assert_eq!(glyphs(&flush), vec![(0.0, 6.0), (5.0, 6.0)]);
}

#[test]
fn mathop_takes_limits_in_display_style() {
    let (model, _) = Model::build(|m| {
        let f = m.symbol('f');
        let body = m.row(vec![f]);
        let op = m.classed(
            AtomClass::Op,
            body,
            Limits::Default {
                display_limits: true,
            },
        );
        let x = m.symbol('x');
        m.scripts(op, None, Some(x))
    });

    // Limits go above and below in display style, so the subscript is centred
    // under the base instead of widening it.
    let display = measured(&model, Style::DISPLAY);
    assert!(close(display.width, 5.0));
    assert!(close(display.ascent, 6.0));

    // Outside display style the same operator keeps its subscript at the side.
    let text = measured(&model, Style::TEXT);
    assert!(text.width > display.width);
    assert!(close(text.ascent, 6.0));
}

#[test]
fn display_limits_stack_a_function_under_it() {
    // `\operatorname{lim}\displaylimits`: the Function's own limits flag stacks
    // the subscript, which is what the writer's rewritten suffix relies on.
    let (plain, _) = Model::build(|m| {
        let lim = m.function("lim", true);
        let x = m.symbol('x');
        m.scripts(lim, None, Some(x))
    });
    let display = measured(&plain, Style::DISPLAY);
    assert!(close(display.width, 15.0));
    assert!(close(display.ascent, 6.0));
    let text = measured(&plain, Style::TEXT);
    assert!(text.width > display.width);

    // The `\varliminf` family wraps the same Function in an Op class; the
    // wrapper's limits decide, and the arrow hangs below the body.
    let (decorated, _) = Model::build(|m| {
        let lim = m.function("lim", false);
        let arrow = m.alloc(
            NodeKind::UnderArrow {
                symbol: '→',
                body: lim,
            },
            Span::default(),
        );
        let op = m.classed(
            AtomClass::Op,
            arrow,
            Limits::Default {
                display_limits: true,
            },
        );
        let x = m.symbol('x');
        m.scripts(op, None, Some(x))
    });
    let display = measured(&decorated, Style::DISPLAY);
    assert!(close(display.width, 15.0));
    assert!(display.descent > 10.0);
}

#[test]
fn mathchoice_lays_out_only_the_branch_for_the_style() {
    let (model, _) = Model::build(|m| {
        let display = {
            let atoms = "iiii".chars().map(|symbol| m.symbol(symbol)).collect();
            m.row(atoms)
        };
        let text = {
            let atoms = "ii".chars().map(|symbol| m.symbol(symbol)).collect();
            m.row(atoms)
        };
        let script = {
            let atom = m.symbol('i');
            m.row(vec![atom])
        };
        let script_script = {
            let atoms = "iii".chars().map(|symbol| m.symbol(symbol)).collect();
            m.row(atoms)
        };
        m.math_choice(display, text, script, script_script)
    });

    // Four atoms in display and text, two in text, one in script and three in
    // scriptscript: each width is one branch's, laid out at the current size.
    assert!(close(measured(&model, Style::DISPLAY).width, 20.0));
    assert!(close(measured(&model, Style::TEXT).width, 10.0));
    assert!(close(measured(&model, Style::SCRIPT).width, 3.5));
    assert!(close(measured(&model, Style::SCRIPT_SCRIPT).width, 7.35));
}

#[test]
fn mathstrut_shape_keeps_the_paren_extent_in_every_style() {
    let (model, _) = Model::build(|m| {
        let a = m.symbol('a');
        let paren = m.symbol('(');
        let strut = m.phantom(paren, PhantomKind::Vertical);
        let b = m.symbol('b');
        m.row(vec![a, strut, b])
    });

    // The strut adds no width, and the row is as tall as the paren at the
    // style's size: 0.6 em up and 0.2 em down.
    let text = measured(&model, Style::TEXT);
    assert!(close(text.width, 10.0));
    assert!(close(text.ascent, 6.0));
    assert!(close(text.descent, 2.0));
    let script = measured(&model, Style::SCRIPT);
    assert!(close(script.width, 7.0));
    assert!(close(script.ascent, 4.2));
    assert!(close(script.descent, 1.4));
}

#[test]
fn color_reaches_nested_glyphs_and_rules() {
    let blue = Color::rgb(0, 0, 255);
    let red = Color::rgb(255, 0, 0);
    let (model, _) = Model::build(|m| {
        let inner = {
            let a = m.symbol('a');
            let body = m.row(vec![a]);
            m.color(blue, body)
        };
        let fraction = {
            let numerator = m.symbol('x');
            let denominator = m.symbol('y');
            m.fraction(numerator, denominator, true)
        };
        let body = m.row(vec![inner, fraction]);
        m.color(red, body)
    });

    let layout = measured(&model, Style::TEXT);
    let glyphs: Vec<Option<Color>> = layout
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Glyph(glyph) => Some(glyph.color),
            _ => None,
        })
        .collect();
    assert_eq!(
        glyphs,
        vec![Some(blue), Some(red), Some(red)],
        "the inner color wins and the outer one reaches the fraction's glyphs"
    );
    let rules: Vec<Option<Color>> = layout
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Rule(rule) => Some(rule.color),
            _ => None,
        })
        .collect();
    assert_eq!(rules, vec![Some(red)], "the fraction bar is colored");
}

#[test]
fn colorbox_background_sits_behind_the_text_and_the_border_keeps_its_color() {
    let background = Color::rgb(255, 0, 0);
    let border = Color::rgb(0, 0, 255);
    let (model, _) = Model::build(|m| {
        let body = m.text("x");
        m.color_box(background, Some(border), body)
    });

    // \fboxsep 0.3em plus \fboxrule 0.04em on every side of 5×8 text.
    let layout = measured(&model, Style::TEXT);
    assert!(close(layout.width, 5.0 + 6.8));
    assert!(close(layout.ascent, 6.0 + 3.4));
    assert!(close(layout.descent, 2.0 + 3.4));

    let Item::Background(fill) = layout.items[0] else {
        panic!("the background must come before the content");
    };
    assert_eq!(fill.color, background);
    assert!(close(fill.x, 0.0));
    assert!(close(fill.y, 0.0));
    assert!(close(fill.width, layout.width));
    assert!(close(fill.height, layout.ascent + layout.descent));

    let rules: Vec<_> = layout
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Rule(rule) => Some(rule),
            _ => None,
        })
        .collect();
    assert_eq!(rules.len(), 4, "a frame has four edges");
    assert!(
        rules.iter().all(|rule| rule.color == Some(border)),
        "the frame uses the border color, not the background"
    );
}
