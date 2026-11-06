//! Box layout: bin cancellation, spacing glue, scripts, fractions, radicals.
//!
//! Local boxes keep the baseline at y = 0 with y growing downward, so a box's
//! ascent is how far its ink reaches above 0 and its descent below. The final
//! `Layout` shifts everything down by the ascent.
//!
//! The script rules follow TeX Appendix G 18a–f as implemented by KaTeX
//! (MIT); the fraction and radical geometry follows formulary's MATH-driven
//! layout (MIT), adapted to this box type. See `THIRD-PARTY.md`.

use crate::atom::{Atom, Variant, math_italic, variant_char};
use crate::class::{AtomClass, gap};
use crate::display::{GlyphItem, Item, Layout, LineItem, RuleItem};
use crate::font::{GlyphId, MathFont, Stretched};
use crate::geometry::Geometry;
use crate::node::{
    Alignment, DelimiterSize, FractionRule, Length, Model, NodeId, NodeKind, PhantomKind, Slot,
    SpaceKind,
};
use crate::span::Span;
use crate::style::Style;
use crate::text::{FontShaper, TextShaper, TextStyle};

/// A layout problem for the caller to surface, anchored to its source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub span: Span,
    pub message: String,
}

impl Diagnostic {
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            span,
            message: message.into(),
        }
    }
}

/// The font, text shaper and model a layout reads.
struct Cx<'a> {
    font: &'a dyn MathFont,
    shaper: &'a dyn TextShaper,
    model: &'a Model,
    /// Reserve a cell for an empty row so an editing host can show its slot.
    placeholders: bool,
}

/// Lays out `node` at `size` in `style`.
pub fn layout(
    font: &dyn MathFont,
    model: &Model,
    root: NodeId,
    size: f32,
    style: Style,
) -> (Layout, Vec<Diagnostic>) {
    let shaper = FontShaper::new(font);
    layout_with_shaper(font, &shaper, model, root, size, style)
}

/// Lays out `root` with the face's own one-glyph-per-character text shaper.
pub fn layout_with_geometry(
    font: &dyn MathFont,
    model: &Model,
    root: NodeId,
    size: f32,
    style: Style,
) -> (Layout, Geometry, Vec<Diagnostic>) {
    let shaper = FontShaper::new(font);
    layout_with_shaper_and_geometry(font, &shaper, model, root, size, style)
}

/// Lays out `root` with a host text shaper.
pub fn layout_with_shaper(
    font: &dyn MathFont,
    shaper: &dyn TextShaper,
    model: &Model,
    root: NodeId,
    size: f32,
    style: Style,
) -> (Layout, Vec<Diagnostic>) {
    let cx = Cx {
        font,
        shaper,
        model,
        placeholders: false,
    };
    let mut diagnostics = Vec::new();
    let body = layout_body(&cx, root, size, style, &mut diagnostics);
    (finish(body), diagnostics)
}

/// Lays out `root` with a host text shaper and builds the editing index over
/// the same boxes. Empty rows reserve a placeholder cell here, so the caret
/// and the host's placeholder occupy the same space as content would.
pub fn layout_with_shaper_and_geometry(
    font: &dyn MathFont,
    shaper: &dyn TextShaper,
    model: &Model,
    root: NodeId,
    size: f32,
    style: Style,
) -> (Layout, Geometry, Vec<Diagnostic>) {
    let cx = Cx {
        font,
        shaper,
        model,
        placeholders: true,
    };
    let mut diagnostics = Vec::new();
    let body = layout_body(&cx, root, size, style, &mut diagnostics);
    let geometry = Geometry::new(model, &body, style.em(font, size));
    (finish(body), geometry, diagnostics)
}

fn layout_body(
    cx: &Cx<'_>,
    root: NodeId,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    layout_node(cx, root, size, style, diagnostics)
}

fn finish(body: MathBox) -> Layout {
    let mut items = Vec::new();
    body.flatten(0.0, body.ascent, &mut items);
    Layout {
        width: body.width,
        ascent: body.ascent,
        descent: body.descent,
        items,
    }
}

/// The cell an empty row reserves in an editing layout, in em.
const PLACEHOLDER_EM: f32 = 0.6;

pub(crate) struct MathBox {
    pub(crate) width: f32,
    pub(crate) ascent: f32,
    pub(crate) descent: f32,
    /// Italic correction, which only a single glyph contributes.
    pub(crate) italic: f32,
    /// The glyph, when the box is exactly one; accents centre on its MATH
    /// top-accent attachment.
    pub(crate) lone_glyph: Option<GlyphId>,
    /// The model node this box stands for, when one exists.
    pub(crate) id: Option<NodeId>,
    /// Ink drawn at this box's own origin.
    pub(crate) items: Vec<Item>,
    /// Positioned child boxes, tagged with the slot they came from.
    pub(crate) children: Vec<Placed>,
    /// The em of an empty row's slot, which has no content to measure.
    pub(crate) empty_em: Option<f32>,
}

pub(crate) struct Placed {
    pub(crate) dx: f32,
    pub(crate) dy: f32,
    pub(crate) slot: Slot,
    pub(crate) child: MathBox,
}

impl MathBox {
    fn empty() -> Self {
        MathBox {
            width: 0.0,
            ascent: 0.0,
            descent: 0.0,
            italic: 0.0,
            lone_glyph: None,
            id: None,
            items: Vec::new(),
            children: Vec::new(),
            empty_em: None,
        }
    }

    fn push_child(&mut self, child: MathBox, dx: f32, dy: f32, slot: Slot) {
        self.children.push(Placed {
            dx,
            dy,
            slot,
            child,
        });
    }

    /// Emits every painted item in the caller's coordinates, children first
    /// so a base's ink precedes a mark drawn over it.
    fn flatten(&self, dx: f32, dy: f32, out: &mut Vec<Item>) {
        for placed in &self.children {
            placed.child.flatten(dx + placed.dx, dy + placed.dy, out);
        }
        for item in &self.items {
            let mut item = *item;
            translate(&mut item, dx, dy);
            out.push(item);
        }
    }
}

fn translate(item: &mut Item, dx: f32, dy: f32) {
    match item {
        Item::Glyph(glyph) => {
            glyph.x += dx;
            glyph.baseline += dy;
        }
        Item::Rule(rule) => {
            rule.x += dx;
            rule.y += dy;
        }
        Item::Line(line) => {
            line.x1 += dx;
            line.y1 += dy;
            line.x2 += dx;
            line.y2 += dy;
        }
    }
}

fn layout_node(
    cx: &Cx<'_>,
    id: NodeId,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let mut out = layout_kind(cx, id, size, style, diagnostics);
    out.id = Some(id);
    out
}

fn layout_kind(
    cx: &Cx<'_>,
    id: NodeId,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let span = cx.model.span(id);
    match cx.model.kind(id) {
        NodeKind::Atom(atom) => layout_atom(cx, atom, span, size, style, diagnostics),
        NodeKind::Row(nodes) => layout_row(cx, nodes, size, style, diagnostics),
        NodeKind::Scripts { base, sup, sub } => {
            layout_scripts(cx, *base, *sup, *sub, size, style, diagnostics)
        }
        NodeKind::Fraction {
            numerator,
            denominator,
            rule,
            style: forced_style,
        } => {
            // `\dfrac` and `\tfrac` replace the surrounding level outright.
            let style = match forced_style {
                Some(level) => Style {
                    size: *level,
                    cramped: false,
                },
                None => style,
            };
            layout_fraction(
                cx,
                *numerator,
                *denominator,
                *rule,
                size,
                style,
                diagnostics,
            )
        }
        NodeKind::Radical { body, degree } => {
            layout_radical(cx, *body, *degree, size, style, diagnostics)
        }
        NodeKind::Delimited { .. } => layout_delimited(cx, id, size, style, diagnostics),
        NodeKind::LargeOp { symbol, .. } => {
            layout_large_op(cx, *symbol, span, size, style, diagnostics)
        }
        NodeKind::Accent { base, symbol } => {
            layout_accent(cx, *base, *symbol, span, size, style, diagnostics)
        }
        NodeKind::Text(text)
        | NodeKind::Composition(text)
        | NodeKind::Error(text)
        | NodeKind::Function { name: text, .. } => {
            layout_text(cx, text, span, size, style, diagnostics)
        }
        NodeKind::Space(kind) => layout_space(cx, *kind, size, style),
        NodeKind::Hspace(length) => layout_hspace(cx, *length, size, style),
        NodeKind::Overline { body } => layout_bar(cx, *body, size, style, diagnostics, true),
        NodeKind::Underline { body } => layout_bar(cx, *body, size, style, diagnostics, false),
        NodeKind::Middle { .. } => MathBox::empty(),
        NodeKind::SizedDelimiter {
            symbol,
            size: delimiter,
            ..
        } => layout_sized_delimiter(cx, *symbol, *delimiter, span, size, style, diagnostics),
        NodeKind::Phantom { body, kind } => {
            layout_phantom(cx, *body, *kind, size, style, diagnostics)
        }
        NodeKind::Smash { body, top, bottom } => {
            layout_smash(cx, *body, *top, *bottom, size, style, diagnostics)
        }
        NodeKind::Boxed { body } => layout_boxed(cx, *body, size, style, diagnostics),
        NodeKind::Cancel { body, up, down } => {
            layout_cancel(cx, *body, *up, *down, size, style, diagnostics)
        }
        NodeKind::Arrow { .. } => layout_arrow(cx, id, size, style, diagnostics),
        NodeKind::HorizontalBrace { body, over } => {
            layout_horizontal_brace(cx, *body, *over, span, size, style, diagnostics)
        }
        NodeKind::OverUnder { base, over, under } => {
            layout_over_under(cx, *base, *over, *under, size, style, diagnostics)
        }
        NodeKind::Table {
            rows,
            columns,
            alignment,
        } => layout_table(cx, rows, *columns, alignment, size, style, diagnostics),
        NodeKind::TableRow { cells } => layout_table_row(cx, cells, size, style, diagnostics),
    }
}

fn layout_atom(
    cx: &Cx<'_>,
    atom: &Atom,
    span: Span,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let em = style.em(cx.font, size);
    // Script levels take the font's script-tuned alternates, as unicode-math
    // does; a face without an `ssty` feature keeps the base glyph.
    let level = match style.size {
        crate::style::Size::Script => 1,
        crate::style::Size::ScriptScript => 2,
        _ => 0,
    };
    let Some(base) = resolve_glyph(cx, atom.symbol, atom.variant) else {
        diagnostics.push(Diagnostic::new(
            span,
            format!("no glyph for U+{:04X}", atom.symbol as u32),
        ));
        return MathBox::empty();
    };
    let glyph = if level == 0 {
        base
    } else {
        cx.font.script_alternate(base, level).unwrap_or(base)
    };
    let (ascent, descent) = match cx.font.ink(glyph) {
        Some(ink) => (ink.top * em, (-ink.bottom * em).max(0.0)),
        None => {
            let (ascent, descent) = cx.font.vertical_metrics();
            (ascent * em, descent * em)
        }
    };
    MathBox {
        width: cx.font.advance(glyph) * em,
        ascent,
        descent,
        italic: cx.font.italic(glyph) * em,
        lone_glyph: Some(glyph),
        id: None,
        children: Vec::new(),
        items: vec![Item::Glyph(GlyphItem {
            glyph,
            x: 0.0,
            baseline: 0.0,
            size: em,
        })],
        empty_em: None,
    }
}

/// The variant's codepoint when the face has it, then the base symbol, then
/// the Mathematical Italic form for ordinary letters.
fn resolve_glyph(cx: &Cx<'_>, symbol: char, variant: Variant) -> Option<GlyphId> {
    let styled = variant_char(symbol, variant);
    if let Some(styled) = styled
        && let Some(glyph) = cx.font.glyph(styled)
    {
        return Some(glyph);
    }
    if variant != Variant::Normal
        && let Some(glyph) = cx.font.glyph(symbol)
    {
        return Some(glyph);
    }
    math_italic(symbol)
        .and_then(|italic| cx.font.glyph(italic))
        .or_else(|| cx.font.glyph(symbol))
}

/// Text mode, named operators, error text and the IME preedit all take their
/// metrics from the host shaper. The run is in em; layout converts it.
fn layout_text(
    cx: &Cx<'_>,
    text: &str,
    span: Span,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let em = style.em(cx.font, size);
    let run = cx.shaper.shape(text, TextStyle::default());
    let mut out = MathBox::empty();
    out.width = run.width * em;
    out.ascent = run.ascent * em;
    out.descent = run.descent * em;
    for glyph in run.glyphs {
        out.items.push(Item::Glyph(GlyphItem {
            glyph: glyph.glyph,
            x: glyph.x * em,
            baseline: -glyph.y * em,
            size: glyph.size * em,
        }));
    }
    for symbol in run.missing {
        diagnostics.push(Diagnostic::new(
            span,
            format!("no glyph for U+{:04X} in text", symbol as u32),
        ));
    }
    out
}

fn layout_row(
    cx: &Cx<'_>,
    nodes: &[NodeId],
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    layout_row_with(cx, nodes, size, style, None, diagnostics)
}

/// `middle` carries the enclosing `\left…\right` target for `\middle`
/// delimiters, which grow with the whole group rather than the local row.
fn layout_row_with(
    cx: &Cx<'_>,
    nodes: &[NodeId],
    size: f32,
    style: Style,
    middle: Option<(f32, f32)>,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let em = style.em(cx.font, size);
    // A spacing command is glue, not an atom: it contributes width but no
    // class, so binary cancellation and gaps see the atoms on either side.
    let mut classes: Vec<AtomClass> = nodes
        .iter()
        .filter(|id| !cx.model.kind(**id).is_space())
        .map(|id| node_class(cx, *id))
        .collect();
    cancel_binary(&mut classes);

    let mut out = MathBox::empty();
    let mut x = 0.0;
    let mut next_class = 0;
    let mut previous: Option<AtomClass> = None;
    for id in nodes {
        if cx.model.kind(*id).is_space() {
            let child = layout_node(cx, *id, size, style, diagnostics);
            x += child.width;
            continue;
        }
        if let Some(left) = previous {
            x += gap(left, classes[next_class], style.tight()).em() * em;
        }
        let child = match (cx.model.kind(*id), middle) {
            (NodeKind::Middle { symbol }, Some((target, axis))) => {
                let mut middle = delimiter_box(
                    cx,
                    *symbol,
                    target,
                    axis,
                    em,
                    cx.model.span(*id),
                    diagnostics,
                );
                // Geometry indexes boundaries by node, so a `\middle` with no
                // id would hide the caret after it.
                middle.id = Some(*id);
                middle
            }
            _ => layout_node(cx, *id, size, style, diagnostics),
        };
        if nodes.len() == 1 {
            out.lone_glyph = child.lone_glyph;
        }
        out.ascent = out.ascent.max(child.ascent);
        out.descent = out.descent.max(child.descent);
        let width = child.width;
        out.push_child(child, x, 0.0, Slot::Body);
        x += width;
        previous = Some(classes[next_class]);
        next_class += 1;
    }
    if nodes.is_empty() {
        out.empty_em = Some(em);
        if cx.placeholders {
            out.width = PLACEHOLDER_EM * em;
            out.ascent = PLACEHOLDER_EM * em;
        }
    } else {
        out.width = x;
    }
    out
}

/// The spacing class a construct contributes to its row. Scripts keep the
/// nucleus's class, a fraction is Inner, a radical is Ord.
fn node_class(cx: &Cx<'_>, id: NodeId) -> AtomClass {
    match cx.model.kind(id) {
        NodeKind::Atom(atom) => atom.class,
        NodeKind::Scripts { base, .. } => node_class(cx, *base),
        NodeKind::Fraction { .. } | NodeKind::Delimited { .. } => AtomClass::Inner,
        // A group is an ordinary atom in TeX, so `a{+}b` keeps the plus Ord.
        NodeKind::Row(_)
        | NodeKind::Radical { .. }
        | NodeKind::Accent { .. }
        | NodeKind::Text(_)
        | NodeKind::Composition(_)
        | NodeKind::Error(_)
        | NodeKind::Space(_)
        | NodeKind::Hspace(_)
        | NodeKind::Overline { .. }
        | NodeKind::Underline { .. }
        | NodeKind::Middle { .. } => AtomClass::Ord,
        NodeKind::LargeOp { .. } | NodeKind::Function { .. } | NodeKind::HorizontalBrace { .. } => {
            AtomClass::Op
        }
        NodeKind::Arrow { .. } => AtomClass::Rel,
        NodeKind::SizedDelimiter { class, .. } => *class,
        NodeKind::Phantom { .. } | NodeKind::Boxed { .. } | NodeKind::Cancel { .. } => {
            AtomClass::Ord
        }
        NodeKind::Table { .. } | NodeKind::TableRow { .. } => AtomClass::Inner,
        NodeKind::Smash { body, .. } => node_class(cx, *body),
        NodeKind::OverUnder { base, .. } => node_class(cx, *base),
    }
}

/// TeX Appendix G Rules 18a–f with the MATH script constants.
fn layout_scripts(
    cx: &Cx<'_>,
    base_node: NodeId,
    sup_node: Option<NodeId>,
    sub_node: Option<NodeId>,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    // Limits are a display-style placement; elsewhere the same scripts sit at
    // the side (TeX's movable limits).
    if takes_limits(cx, base_node)
        && (style.display() || forces_limits(cx, base_node))
        && (sup_node.is_some() || sub_node.is_some())
    {
        return layout_limits(cx, base_node, sup_node, sub_node, size, style, diagnostics);
    }

    let em = style.em(cx.font, size);
    let constants = cx.font.constants();
    let base = layout_node(cx, base_node, size, style, diagnostics);

    let sup = sup_node.map(|id| layout_node(cx, id, size, style.sup(), diagnostics));
    let sub = sub_node.map(|id| layout_node(cx, id, size, style.sub(), diagnostics));
    if sup.is_none() && sub.is_none() {
        return base;
    }

    // KaTeX's `isCharacterBox` admits ordinary symbols but not operators, so a
    // large operator's scripts follow its height and depth (Rule 18a).
    let is_character = matches!(
        cx.model.kind(base_node),
        NodeKind::Atom(atom) if atom.class != AtomClass::Op
    );
    let italic = base.italic;
    let x_height = cx.font.x_height() * em;
    let min_sup_shift = if style.cramped {
        constants.superscript_shift_up_cramped
    } else {
        constants.superscript_shift_up
    } * em;
    let script_space = constants.space_after_script * em;

    // Rule 18a: a character nucleus has no drop; a box nucleus drops by the
    // font's baseline-drop constants.
    let mut sup_shift = 0.0;
    let mut sub_shift = 0.0;
    if !is_character {
        if sup.is_some() {
            sup_shift = base.ascent - constants.superscript_baseline_drop_max * em;
        }
        if sub.is_some() {
            sub_shift = base.descent + constants.subscript_baseline_drop_min * em;
        }
    }

    match (&sup, &sub) {
        (Some(sup_box), Some(sub_box)) => {
            sup_shift = sup_shift
                .max(min_sup_shift)
                .max(sup_box.descent + 0.25 * x_height);
            sub_shift = sub_shift.max(constants.sub_superscript_gap_min * em);

            // Rule 18e: keep 4ξ8 between the superscript's bottom and the
            // subscript's top, then lift the superscript by the remaining
            // four-fifths x-height.
            let clearance = 4.0 * constants.fraction_rule_thickness * em;
            let gap = (sup_shift - sup_box.descent) - (sub_box.ascent - sub_shift);
            if gap < clearance {
                sub_shift = clearance - (sup_shift - sup_box.descent) + sub_box.ascent;
                let psi = 0.8 * x_height - (sup_shift - sup_box.descent);
                if psi > 0.0 {
                    sup_shift += psi;
                    sub_shift -= psi;
                }
            }
        }
        (Some(sup_box), None) => {
            sup_shift = sup_shift
                .max(min_sup_shift)
                .max(sup_box.descent + 0.25 * x_height);
        }
        (None, Some(sub_box)) => {
            sub_shift = sub_shift
                .max(constants.subscript_shift_down * em)
                .max(sub_box.ascent - 0.8 * x_height);
        }
        (None, None) => unreachable!("checked above"),
    }

    // Rule 18f: a character nucleus grows by its italic correction, and the
    // subscript is placed at the original advance so it does not ride on it.
    let nucleus = base.width + italic;
    let sup_x = nucleus;
    let sub_x = base.width;
    let mut width = nucleus;
    if let Some(script) = &sup {
        width = width.max(sup_x + script.width + script_space);
    }
    if let Some(script) = &sub {
        width = width.max(sub_x + script.width + script_space);
    }

    let mut ascent = base.ascent;
    let mut descent = base.descent;
    if let Some(script) = &sup {
        ascent = ascent.max(sup_shift + script.ascent);
        descent = descent.max(script.descent - sup_shift);
    }
    if let Some(script) = &sub {
        ascent = ascent.max(script.ascent - sub_shift);
        descent = descent.max(sub_shift + script.descent);
    }

    let mut out = MathBox {
        width,
        ascent,
        descent,
        italic: 0.0,
        lone_glyph: None,
        id: None,
        children: Vec::new(),
        items: Vec::new(),
        empty_em: None,
    };
    out.push_child(base, 0.0, 0.0, Slot::Base);
    if let Some(script) = sup {
        out.push_child(script, sup_x, -sup_shift, Slot::Superscript);
    }
    if let Some(script) = sub {
        out.push_child(script, sub_x, sub_shift, Slot::Subscript);
    }
    out
}

fn takes_limits(cx: &Cx<'_>, id: NodeId) -> bool {
    matches!(
        cx.model.kind(id),
        NodeKind::LargeOp { limits: true, .. }
            | NodeKind::Function { limits: true, .. }
            | NodeKind::HorizontalBrace { .. }
    )
}

/// Constructs whose scripts stay above and below in every style, unlike a
/// large operator's movable limits.
fn forces_limits(cx: &Cx<'_>, id: NodeId) -> bool {
    matches!(cx.model.kind(id), NodeKind::HorizontalBrace { .. })
}

/// TeX Rule 13: limits above and below a display-style large operator, each
/// clearing the operator by its baseline rise/drop or gap, whichever is larger.
fn layout_limits(
    cx: &Cx<'_>,
    base_node: NodeId,
    sup_node: Option<NodeId>,
    sub_node: Option<NodeId>,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let em = style.em(cx.font, size);
    let constants = cx.font.constants();
    let base = layout_node(cx, base_node, size, style, diagnostics);
    let sup = sup_node.map(|id| layout_node(cx, id, size, style.sup(), diagnostics));
    let sub = sub_node.map(|id| layout_node(cx, id, size, style.sub(), diagnostics));

    let width = base
        .width
        .max(sup.as_ref().map_or(0.0, |script| script.width))
        .max(sub.as_ref().map_or(0.0, |script| script.width));
    let base_ascent = base.ascent;
    let base_descent = base.descent;
    let mut out = MathBox {
        width,
        ascent: base_ascent,
        descent: base_descent,
        italic: 0.0,
        lone_glyph: None,
        id: None,
        children: Vec::new(),
        items: Vec::new(),
        empty_em: None,
    };
    let base_dx = (width - base.width) / 2.0;
    out.push_child(base, base_dx, 0.0, Slot::Base);

    if let Some(script) = sup {
        let shift = base_ascent
            + (constants.upper_limit_baseline_rise_min * em)
                .max(constants.upper_limit_gap_min * em + script.descent);
        out.ascent = out.ascent.max(shift + script.ascent);
        let dx = (width - script.width) / 2.0;
        out.push_child(script, dx, -shift, Slot::Superscript);
    }
    if let Some(script) = sub {
        let shift = base_descent
            + (constants.lower_limit_baseline_drop_min * em)
                .max(constants.lower_limit_gap_min * em + script.ascent);
        out.descent = out.descent.max(shift + script.descent);
        let dx = (width - script.width) / 2.0;
        out.push_child(script, dx, shift, Slot::Subscript);
    }
    out
}

/// A delimiter grown symmetrically about the math axis (TeX Rule 19): the
/// body's larger reach from the axis sets the half-height, and a variant that
/// overshoots is centred on that target.
fn layout_delimited(
    cx: &Cx<'_>,
    id: NodeId,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let NodeKind::Delimited { left, body, right } = cx.model.kind(id) else {
        unreachable!("layout_node dispatched on Delimited")
    };
    let span = cx.model.span(id);
    let em = style.em(cx.font, size);
    let axis = cx.font.constants().axis_height * em;
    let children = match cx.model.kind(*body) {
        NodeKind::Row(children) => Some(children.as_slice()),
        _ => None,
    };
    let has_middle = children.is_some_and(|children| {
        children
            .iter()
            .any(|child| matches!(cx.model.kind(*child), NodeKind::Middle { .. }))
    });
    let (body, target) = if let (true, Some(children)) = (has_middle, children) {
        // Size the middles from the body without them; they cannot exceed it.
        let mut scratch = Vec::new();
        let probe = layout_node(cx, *body, size, style, &mut scratch);
        let above = (probe.ascent - axis).max(probe.descent + axis).max(0.0);
        let target = 2.0 * above;
        let mut row = layout_row_with(cx, children, size, style, Some((target, axis)), diagnostics);
        // Geometry needs the row's id to offer caret stops inside a body that
        // contains `\middle`; the plain row path sets it in `layout_node`.
        row.id = Some(*body);
        (row, target)
    } else {
        let body = layout_node(cx, *body, size, style, diagnostics);
        let above = (body.ascent - axis).max(body.descent + axis).max(0.0);
        (body, 2.0 * above)
    };

    let left = left.map(|symbol| delimiter_box(cx, symbol, target, axis, em, span, diagnostics));
    let right = right.map(|symbol| delimiter_box(cx, symbol, target, axis, em, span, diagnostics));
    let left_width = left.as_ref().map_or(0.0, |delimiter| delimiter.width);
    let right_width = right.as_ref().map_or(0.0, |delimiter| delimiter.width);
    let body_width = body.width;

    let mut out = MathBox {
        width: left_width + body_width + right_width,
        ascent: body.ascent,
        descent: body.descent,
        italic: 0.0,
        lone_glyph: None,
        id: None,
        children: Vec::new(),
        items: Vec::new(),
        empty_em: None,
    };
    if let Some(delimiter) = left {
        out.ascent = out.ascent.max(delimiter.ascent);
        out.descent = out.descent.max(delimiter.descent);
        out.push_child(delimiter, 0.0, 0.0, Slot::Body);
    }
    out.push_child(body, left_width, 0.0, Slot::Body);
    if let Some(delimiter) = right {
        out.ascent = out.ascent.max(delimiter.ascent);
        out.descent = out.descent.max(delimiter.descent);
        out.push_child(delimiter, left_width + body_width, 0.0, Slot::Body);
    }
    out
}

fn delimiter_box(
    cx: &Cx<'_>,
    symbol: char,
    target: f32,
    axis: f32,
    em: f32,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let Some(glyph) = cx.font.glyph(symbol) else {
        diagnostics.push(Diagnostic::new(
            span,
            format!("no glyph for delimiter U+{:04X}", symbol as u32),
        ));
        return MathBox::empty();
    };
    if target <= 0.0 {
        let Some(ink) = cx.font.ink(glyph) else {
            return MathBox::empty();
        };
        return MathBox {
            width: cx.font.advance(glyph) * em,
            ascent: ink.top * em,
            descent: (-ink.bottom * em).max(0.0),
            italic: 0.0,
            lone_glyph: Some(glyph),
            id: None,
            children: Vec::new(),
            items: vec![Item::Glyph(GlyphItem {
                glyph,
                x: 0.0,
                baseline: 0.0,
                size: em,
            })],
            empty_em: None,
        };
    }

    let stretched = cx.font.stretch_vertical(glyph, target / em);
    let (height, advance) = stretch_extent(cx, &stretched, em);
    let ascent = axis + target / 2.0 + (height - target).max(0.0) / 2.0;
    let descent = (height - ascent).max(0.0);
    let mut out = MathBox {
        width: advance,
        ascent,
        descent,
        italic: 0.0,
        lone_glyph: None,
        id: None,
        children: Vec::new(),
        items: Vec::new(),
        empty_em: None,
    };
    emit_stretched(cx, &stretched, 0.0, -ascent, em, &mut out.items);
    out
}

/// A large operator: grown to DisplayOperatorMinHeight in display style and
/// centred on the math axis in every style (Rule 13).
fn layout_large_op(
    cx: &Cx<'_>,
    symbol: char,
    span: Span,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let em = style.em(cx.font, size);
    let constants = cx.font.constants();
    let Some(glyph) = cx.font.glyph(symbol) else {
        diagnostics.push(Diagnostic::new(
            span,
            format!("no glyph for large operator U+{:04X}", symbol as u32),
        ));
        return MathBox::empty();
    };
    let stretched = if style.display() {
        cx.font
            .stretch_vertical(glyph, constants.display_operator_min_height)
    } else {
        Stretched::Glyph(glyph)
    };
    let (height, advance) = stretch_extent(cx, &stretched, em);
    let ascent = constants.axis_height * em + height / 2.0;
    let descent = (height - ascent).max(0.0);
    let mut out = MathBox {
        width: advance,
        ascent,
        descent,
        italic: cx.font.italic(glyph) * em,
        lone_glyph: None,
        id: None,
        children: Vec::new(),
        items: Vec::new(),
        empty_em: None,
    };
    emit_stretched(cx, &stretched, 0.0, -ascent, em, &mut out.items);
    out
}

/// MATH accent placement: the mark's baseline sits at AccentBaseHeight above
/// the baseline and rises further for a taller base; the two glyphs'
/// top-accent attachments decide the horizontal offset.
fn layout_accent(
    cx: &Cx<'_>,
    base_node: NodeId,
    symbol: char,
    span: Span,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let em = style.em(cx.font, size);
    let constants = cx.font.constants();
    // Accents keep their size: no script-level bump.
    let base = layout_node(cx, base_node, size, style, diagnostics);
    let Some(glyph) = cx.font.glyph(symbol) else {
        diagnostics.push(Diagnostic::new(
            span,
            format!("no glyph for accent U+{:04X}", symbol as u32),
        ));
        return base;
    };
    let (ascent, _) = match cx.font.ink(glyph) {
        Some(ink) => (ink.top * em, (-ink.bottom * em).max(0.0)),
        None => {
            let (ascent, descent) = cx.font.vertical_metrics();
            (ascent * em, descent * em)
        }
    };
    let mark_width = cx.font.advance(glyph) * em;
    let shift_up = (base.ascent - constants.accent_base_height * em).max(0.0);
    let attach = |glyph: GlyphId, width: f32| {
        cx.font
            .top_accent_attachment(glyph)
            .map_or(width / 2.0, |value| value * em)
    };
    let width = base.width.max(mark_width);
    let base_dx = (width - base.width) / 2.0;
    let base_attach = base_dx
        + base
            .lone_glyph
            .map_or(base.width / 2.0, |glyph| attach(glyph, base.width));
    let dx = base_attach - attach(glyph, mark_width);

    let mut out = MathBox {
        width,
        ascent: base.ascent.max(shift_up + ascent),
        descent: base.descent,
        italic: 0.0,
        lone_glyph: None,
        id: None,
        children: Vec::new(),
        items: Vec::new(),
        empty_em: None,
    };
    out.push_child(base, base_dx, 0.0, Slot::Base);
    out.items.push(Item::Glyph(GlyphItem {
        glyph,
        x: dx,
        baseline: -shift_up,
        size: em,
    }));
    out
}

fn layout_space(cx: &Cx<'_>, kind: SpaceKind, size: f32, style: Style) -> MathBox {
    let em = style.em(cx.font, size);
    MathBox {
        width: kind.em() * em,
        ..MathBox::empty()
    }
}

fn layout_hspace(cx: &Cx<'_>, length: Length, size: f32, style: Style) -> MathBox {
    let em = style.em(cx.font, size);
    MathBox {
        width: length.em(em, cx.font.x_height()),
        ..MathBox::empty()
    }
}

/// `\cancel`: the body's box with one diagonal, or both.
fn layout_cancel(
    cx: &Cx<'_>,
    body_node: NodeId,
    up: bool,
    down: bool,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let em = style.em(cx.font, size);
    let body = layout_node(cx, body_node, size, style, diagnostics);
    let mut out = MathBox {
        width: body.width,
        ascent: body.ascent,
        descent: body.descent,
        ..MathBox::empty()
    };
    let top = -body.ascent;
    let bottom = body.descent;
    let thickness = 0.04 * em;
    let mut diagonal = |x1: f32, y1: f32, x2: f32, y2: f32| {
        out.items.push(Item::Line(LineItem {
            x1,
            y1,
            x2,
            y2,
            thickness,
        }));
    };
    if up {
        diagonal(0.0, bottom, body.width, top);
    }
    if down {
        diagonal(0.0, top, body.width, bottom);
    }
    out.push_child(body, 0.0, 0.0, Slot::Body);
    out
}

/// `\overline` and `\underline`: a rule cleared from the base by the MATH bar
/// constants, spanning exactly the base's width.
fn layout_bar(
    cx: &Cx<'_>,
    body_node: NodeId,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
    over: bool,
) -> MathBox {
    let em = style.em(cx.font, size);
    let constants = cx.font.constants();
    let body = layout_node(cx, body_node, size, style, diagnostics);
    let (gap, thickness, extra) = if over {
        (
            constants.overbar_vertical_gap * em,
            constants.overbar_rule_thickness * em,
            constants.overbar_extra_ascender * em,
        )
    } else {
        (
            constants.underbar_vertical_gap * em,
            constants.underbar_rule_thickness * em,
            constants.underbar_extra_descender * em,
        )
    };
    let width = body.width;
    let mut out = MathBox {
        width,
        ascent: body.ascent,
        descent: body.descent,
        italic: 0.0,
        lone_glyph: None,
        id: None,
        children: Vec::new(),
        items: Vec::new(),
        empty_em: None,
    };
    let rule_y = if over {
        out.ascent = body.ascent + gap + thickness + extra;
        -(body.ascent + gap + thickness)
    } else {
        out.descent = body.descent + gap + thickness + extra;
        body.descent + gap
    };
    out.push_child(body, 0.0, 0.0, Slot::Body);
    out.items.push(Item::Rule(RuleItem {
        x: 0.0,
        y: rule_y,
        width,
        height: thickness,
    }));
    out
}

fn layout_sized_delimiter(
    cx: &Cx<'_>,
    symbol: char,
    delimiter: DelimiterSize,
    span: Span,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let em = style.em(cx.font, size);
    let axis = cx.font.constants().axis_height * em;
    delimiter_box(
        cx,
        symbol,
        delimiter.height() * em,
        axis,
        em,
        span,
        diagnostics,
    )
}

fn layout_phantom(
    cx: &Cx<'_>,
    body_node: NodeId,
    kind: PhantomKind,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let body = layout_node(cx, body_node, size, style, diagnostics);
    let (width, ascent, descent) = match kind {
        PhantomKind::Full => (body.width, body.ascent, body.descent),
        PhantomKind::Horizontal => (body.width, 0.0, 0.0),
        PhantomKind::Vertical => (0.0, body.ascent, body.descent),
    };
    MathBox {
        width,
        ascent,
        descent,
        ..MathBox::empty()
    }
}

fn layout_smash(
    cx: &Cx<'_>,
    body_node: NodeId,
    top: bool,
    bottom: bool,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let body = layout_node(cx, body_node, size, style, diagnostics);
    MathBox {
        ascent: if top { 0.0 } else { body.ascent },
        descent: if bottom { 0.0 } else { body.descent },
        ..body
    }
}

/// `\boxed`: a 3pt pad and a 0.4pt rule at 10pt, TeX's `\fbox` defaults,
/// scaled with the size.
fn layout_boxed(
    cx: &Cx<'_>,
    body_node: NodeId,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let em = style.em(cx.font, size);
    let padding = 0.3 * em;
    let thickness = 0.04 * em;
    let body = layout_node(cx, body_node, size, style, diagnostics);
    let width = body.width + 2.0 * (padding + thickness);
    let ascent = body.ascent + padding + thickness;
    let descent = body.descent + padding + thickness;
    let mut out = MathBox {
        width,
        ascent,
        descent,
        italic: 0.0,
        lone_glyph: None,
        id: None,
        children: Vec::new(),
        items: Vec::new(),
        empty_em: None,
    };
    out.push_child(body, padding + thickness, 0.0, Slot::Body);
    let height = ascent + descent;
    out.items.push(Item::Rule(RuleItem {
        x: 0.0,
        y: -ascent,
        width,
        height: thickness,
    }));
    out.items.push(Item::Rule(RuleItem {
        x: 0.0,
        y: descent - thickness,
        width,
        height: thickness,
    }));
    out.items.push(Item::Rule(RuleItem {
        x: 0.0,
        y: -ascent,
        width: thickness,
        height,
    }));
    out.items.push(Item::Rule(RuleItem {
        x: width - thickness,
        y: -ascent,
        width: thickness,
        height,
    }));
    out
}

/// `\overset` / `\underset`: the annotation at script size, cleared from the
/// base by the MATH bar gaps, centred as a whole.
fn layout_over_under(
    cx: &Cx<'_>,
    base_node: NodeId,
    over_node: Option<NodeId>,
    under_node: Option<NodeId>,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let em = style.em(cx.font, size);
    let constants = cx.font.constants();
    let base = layout_node(cx, base_node, size, style, diagnostics);
    let over = over_node.map(|id| layout_node(cx, id, size, style.sup(), diagnostics));
    let under = under_node.map(|id| layout_node(cx, id, size, style.sub(), diagnostics));
    let width = base
        .width
        .max(over.as_ref().map_or(0.0, |box_| box_.width))
        .max(under.as_ref().map_or(0.0, |box_| box_.width));
    let mut out = MathBox {
        width,
        ascent: base.ascent,
        descent: base.descent,
        italic: 0.0,
        lone_glyph: None,
        id: None,
        children: Vec::new(),
        items: Vec::new(),
        empty_em: None,
    };
    let base_dx = (width - base.width) / 2.0;
    out.push_child(base, base_dx, 0.0, Slot::Base);
    if let Some(over) = over {
        let shift = out.ascent + constants.overbar_vertical_gap * em + over.descent;
        out.ascent = out
            .ascent
            .max(shift + over.ascent + constants.overbar_extra_ascender * em);
        let dx = (width - over.width) / 2.0;
        out.push_child(over, dx, -shift, Slot::Over);
    }
    if let Some(under) = under {
        let shift = out.descent + constants.underbar_vertical_gap * em + under.ascent;
        out.descent = out
            .descent
            .max(shift + under.descent + constants.underbar_extra_descender * em);
        let dx = (width - under.width) / 2.0;
        out.push_child(under, dx, shift, Slot::Under);
    }
    out
}

/// Generalized fraction per the MATH constants: preferred shifts, grown until
/// each part clears the rule by its minimum gap; the rule sits on the axis.
fn layout_fraction(
    cx: &Cx<'_>,
    numerator: NodeId,
    denominator: NodeId,
    rule: FractionRule,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let em = style.em(cx.font, size);
    let constants = cx.font.constants();
    let numerator = layout_node(cx, numerator, size, style.frac_num(), diagnostics);
    let denominator = layout_node(cx, denominator, size, style.frac_den(), diagnostics);
    let axis = constants.axis_height * em;
    let width = numerator.width.max(denominator.width);
    let mut out = MathBox {
        width,
        ..MathBox::empty()
    };

    if !matches!(rule, FractionRule::None) {
        let thickness = match rule {
            FractionRule::Custom(length) => length.em(em, cx.font.x_height()),
            _ => constants.fraction_rule_thickness * em,
        }
        .max(0.0);
        let (shift_up, shift_down, gap_above, gap_below) = if style.display() {
            (
                constants.fraction_numerator_display_shift_up,
                constants.fraction_denominator_display_shift_down,
                constants.fraction_numerator_display_gap_min,
                constants.fraction_denominator_display_gap_min,
            )
        } else {
            (
                constants.fraction_numerator_shift_up,
                constants.fraction_denominator_shift_down,
                constants.fraction_numerator_gap_min,
                constants.fraction_denominator_gap_min,
            )
        };
        let numerator_shift =
            (shift_up * em).max(axis + thickness / 2.0 + gap_above * em + numerator.descent);
        let denominator_shift =
            (shift_down * em).max(gap_below * em + thickness / 2.0 - axis + denominator.ascent);

        out.ascent = numerator_shift + numerator.ascent;
        out.descent = denominator_shift + denominator.descent;
        let numerator_dx = (width - numerator.width) / 2.0;
        let denominator_dx = (width - denominator.width) / 2.0;
        out.push_child(numerator, numerator_dx, -numerator_shift, Slot::Numerator);
        out.push_child(
            denominator,
            denominator_dx,
            denominator_shift,
            Slot::Denominator,
        );
        out.items.push(Item::Rule(RuleItem {
            x: 0.0,
            y: -(axis + thickness / 2.0),
            width,
            height: thickness,
        }));
    } else {
        let (mut top_shift, mut bottom_shift, gap_min) = if style.display() {
            (
                constants.stack_top_display_shift_up * em,
                constants.stack_bottom_display_shift_down * em,
                constants.stack_display_gap_min * em,
            )
        } else {
            (
                constants.stack_top_shift_up * em,
                constants.stack_bottom_shift_down * em,
                constants.stack_gap_min * em,
            )
        };
        let clearance = (top_shift - numerator.descent) + (bottom_shift - denominator.ascent);
        if clearance < gap_min {
            let bump = (gap_min - clearance) / 2.0;
            top_shift += bump;
            bottom_shift += bump;
        }
        out.ascent = top_shift + numerator.ascent;
        out.descent = bottom_shift + denominator.descent;
        let numerator_dx = (width - numerator.width) / 2.0;
        let denominator_dx = (width - denominator.width) / 2.0;
        out.push_child(numerator, numerator_dx, -top_shift, Slot::Numerator);
        out.push_child(denominator, denominator_dx, bottom_shift, Slot::Denominator);
    }
    out
}

const RADICAL: char = '\u{221A}';

/// The radicand is cramped; the surd grows to cover radicand + gap + rule, its
/// ink top carries the overbar, and the degree sits on the raised corner.
fn layout_radical(
    cx: &Cx<'_>,
    body: NodeId,
    degree: Option<NodeId>,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let em = style.em(cx.font, size);
    let constants = cx.font.constants();
    let mut body = layout_node(cx, body, size, style.cramped(), diagnostics);
    let x_height = cx.font.x_height() * em;
    if body.ascent + body.descent == 0.0 {
        body.ascent = x_height;
    }

    let gap_min = if style.display() {
        constants.radical_display_vertical_gap
    } else {
        constants.radical_vertical_gap
    } * em;
    let thickness = constants.radical_rule_thickness * em;
    let target = body.ascent + body.descent + gap_min + thickness;
    let stretched = cx
        .font
        .glyph(RADICAL)
        .map(|glyph| cx.font.stretch_vertical(glyph, target / em));
    let (surd_height, surd_advance) = stretched
        .as_ref()
        .map_or((0.0, 0.0), |stretched| stretch_extent(cx, stretched, em));

    // A taller-than-needed surd centres its excess: half above the gap, half
    // below the radicand (Rule 11).
    let gap = gap_min + (surd_height - target).max(0.0) / 2.0;
    let bar_top = -(body.ascent + gap + thickness);

    let mut out = MathBox {
        width: 0.0,
        ascent: body.ascent + gap + thickness + constants.radical_extra_ascender * em,
        descent: body.descent.max(bar_top + surd_height),
        italic: 0.0,
        lone_glyph: None,
        id: None,
        children: Vec::new(),
        items: Vec::new(),
        empty_em: None,
    };

    let mut x = 0.0;
    if let Some(degree_node) = degree {
        let degree = layout_node(cx, degree_node, size, Style::SCRIPT_SCRIPT, diagnostics);
        x += constants.radical_kern_before_degree * em;
        let glyph_bottom = bar_top + surd_height;
        let baseline =
            glyph_bottom - constants.radical_degree_bottom_raise * surd_height - degree.descent;
        out.ascent = out.ascent.max(degree.ascent - baseline);
        out.descent = out.descent.max(baseline + degree.descent);
        let degree_width = degree.width;
        out.push_child(degree, x, baseline, Slot::Degree);
        x += degree_width + constants.radical_kern_after_degree * em;
        x = x.max(0.0);
    }

    if let Some(stretched) = &stretched {
        emit_stretched(cx, stretched, x, bar_top, em, &mut out.items);
    }
    x += surd_advance;
    let body_width = body.width;
    out.items.push(Item::Rule(RuleItem {
        x,
        y: bar_top,
        width: body_width,
        height: thickness,
    }));
    out.push_child(body, x, 0.0, Slot::Body);
    out.width = x + body_width;
    out
}

/// A matrix-style table: cells share column widths and row baselines.
fn layout_table(
    cx: &Cx<'_>,
    rows: &[NodeId],
    columns: usize,
    alignment: &[Alignment],
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let em = style.em(cx.font, size);
    let column_gap = 0.5 * em;
    let row_gap = 0.5 * em;

    // Lay every cell out first; column widths need the whole table.
    let mut grid: Vec<(NodeId, Vec<MathBox>)> = Vec::new();
    for row_id in rows {
        let NodeKind::TableRow { cells } = cx.model.kind(*row_id) else {
            diagnostics.push(Diagnostic::new(
                cx.model.span(*row_id),
                "a table row must be a row of cells",
            ));
            continue;
        };
        let mut laid = Vec::with_capacity(columns);
        for cell in cells {
            laid.push(layout_node(cx, *cell, size, style, diagnostics));
        }
        while laid.len() < columns {
            laid.push(MathBox::empty());
        }
        laid.truncate(columns);
        grid.push((*row_id, laid));
    }

    let mut widths = vec![0.0f32; columns];
    for (_, cells) in &grid {
        for (column, cell) in cells.iter().enumerate() {
            widths[column] = widths[column].max(cell.width);
        }
    }
    let width = widths.iter().sum::<f32>() + column_gap * columns.saturating_sub(1) as f32;

    let mut out = MathBox {
        width,
        ..MathBox::empty()
    };
    let mut baseline = 0.0;
    let mut previous_descent = 0.0;
    for (index, (row_id, cells)) in grid.into_iter().enumerate() {
        let ascent = cells.iter().map(|cell| cell.ascent).fold(0.0, f32::max);
        let descent = cells.iter().map(|cell| cell.descent).fold(0.0, f32::max);
        if index > 0 {
            baseline += previous_descent + row_gap + ascent;
        }
        previous_descent = descent;

        let mut row = MathBox {
            width,
            ascent,
            descent,
            ..MathBox::empty()
        };
        row.id = Some(row_id);
        let mut x = 0.0;
        for (column, cell) in cells.into_iter().enumerate() {
            let align = alignment.get(column).copied().unwrap_or_default();
            let offset = match align {
                Alignment::Left => 0.0,
                Alignment::Center => (widths[column] - cell.width) / 2.0,
                Alignment::Right => widths[column] - cell.width,
            };
            row.push_child(cell, x + offset, 0.0, Slot::Cell);
            x += widths[column] + column_gap;
        }
        out.ascent = out.ascent.max(ascent - baseline);
        out.descent = out.descent.max(descent + baseline);
        // Offsets are y-down, so a later row sits at a positive baseline.
        out.push_child(row, 0.0, baseline, Slot::Row);
    }
    out
}

/// A table row on its own: cells keep their own widths in one line.
fn layout_table_row(
    cx: &Cx<'_>,
    cells: &[NodeId],
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let em = style.em(cx.font, size);
    let gap = 0.5 * em;
    let mut out = MathBox::empty();
    let mut x = 0.0;
    for cell in cells {
        let cell = layout_node(cx, *cell, size, style, diagnostics);
        out.ascent = out.ascent.max(cell.ascent);
        out.descent = out.descent.max(cell.descent);
        let width = cell.width;
        out.push_child(cell, x, 0.0, Slot::Cell);
        x += width + gap;
    }
    out.width = (x - gap).max(0.0);
    out
}

/// The ink height and advance of a stretched glyph, in layout units.
fn stretch_extent(cx: &Cx<'_>, stretched: &Stretched, em: f32) -> (f32, f32) {
    match stretched {
        Stretched::Glyph(glyph) => {
            let height = cx
                .font
                .ink(*glyph)
                .map_or(0.0, |ink| (ink.top - ink.bottom) * em);
            (height, cx.font.advance(*glyph) * em)
        }
        Stretched::Assembly { parts, extent } => {
            let advance = parts
                .iter()
                .map(|(glyph, _)| cx.font.advance(*glyph))
                .fold(0.0, f32::max);
            (extent * em, advance * em)
        }
    }
}

/// Emits a stretched glyph with its ink top at `top`.
fn emit_stretched(
    cx: &Cx<'_>,
    stretched: &Stretched,
    x: f32,
    top: f32,
    em: f32,
    items: &mut Vec<Item>,
) {
    match stretched {
        Stretched::Glyph(glyph) => {
            if let Some(ink) = cx.font.ink(*glyph) {
                items.push(Item::Glyph(GlyphItem {
                    glyph: *glyph,
                    x,
                    baseline: top + ink.top * em,
                    size: em,
                }));
            }
        }
        Stretched::Assembly { parts, extent } => {
            let bottom = top + extent * em;
            for (glyph, offset) in parts {
                if let Some(ink) = cx.font.ink(*glyph) {
                    items.push(Item::Glyph(GlyphItem {
                        glyph: *glyph,
                        x,
                        baseline: bottom - offset * em + ink.bottom * em,
                        size: em,
                    }));
                }
            }
        }
    }
}

/// A horizontally stretched glyph: its width and vertical reach in layout
/// units.
fn horizontal_metrics(cx: &Cx<'_>, stretched: &Stretched, em: f32) -> (f32, f32, f32) {
    let glyphs: Vec<GlyphId> = match stretched {
        Stretched::Glyph(glyph) => vec![*glyph],
        Stretched::Assembly { parts, .. } => parts.iter().map(|(glyph, _)| *glyph).collect(),
    };
    let width = match stretched {
        Stretched::Glyph(glyph) => cx.font.advance(*glyph) * em,
        Stretched::Assembly { extent, .. } => extent * em,
    };
    let mut ascent: f32 = 0.0;
    let mut descent: f32 = 0.0;
    for glyph in glyphs {
        if let Some(ink) = cx.font.ink(glyph) {
            ascent = ascent.max(ink.top * em);
            descent = descent.max(-ink.bottom * em);
        }
    }
    (width, ascent, descent)
}

/// Emits a horizontally stretched glyph with its baseline at `baseline`.
fn emit_stretched_horizontal(
    stretched: &Stretched,
    x: f32,
    baseline: f32,
    em: f32,
    items: &mut Vec<Item>,
) {
    match stretched {
        Stretched::Glyph(glyph) => items.push(Item::Glyph(GlyphItem {
            glyph: *glyph,
            x,
            baseline,
            size: em,
        })),
        Stretched::Assembly { parts, .. } => {
            for (glyph, offset) in parts {
                items.push(Item::Glyph(GlyphItem {
                    glyph: *glyph,
                    x: x + offset * em,
                    baseline,
                    size: em,
                }));
            }
        }
    }
}

/// `\xrightarrow` and `\xleftarrow`: a horizontal arrow stretched past its
/// labels, which sit above and below it.
fn layout_arrow(
    cx: &Cx<'_>,
    id: NodeId,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let NodeKind::Arrow {
        symbol,
        over,
        under,
    } = cx.model.kind(id)
    else {
        unreachable!("layout_node dispatched on Arrow")
    };
    let (symbol, over_node, under_node, span) = (*symbol, *over, *under, cx.model.span(id));
    let em = style.em(cx.font, size);
    let over = over_node.map(|id| layout_node(cx, id, size, style.sup(), diagnostics));
    let under = under_node.map(|id| layout_node(cx, id, size, style.sub(), diagnostics));
    let gap = 0.15 * em;
    let label_width = over
        .as_ref()
        .map_or(0.0, |label| label.width)
        .max(under.as_ref().map_or(0.0, |label| label.width));
    let mut width = (label_width + 0.5 * em).max(em);

    let Some(glyph) = cx.font.glyph(symbol) else {
        diagnostics.push(Diagnostic::new(
            span,
            format!("font has no glyph for U+{:04X}", symbol as u32),
        ));
        return MathBox {
            width,
            ..MathBox::empty()
        };
    };
    let stretched = cx.font.stretch_horizontal(glyph, width / em);
    let (arrow_width, arrow_ascent, arrow_descent) = horizontal_metrics(cx, &stretched, em);
    width = width.max(arrow_width);
    let mut out = MathBox {
        width,
        ascent: arrow_ascent,
        descent: arrow_descent,
        ..MathBox::empty()
    };
    emit_stretched_horizontal(
        &stretched,
        (width - arrow_width) / 2.0,
        0.0,
        em,
        &mut out.items,
    );

    if let Some(over) = over {
        let shift = out.ascent + gap + over.descent;
        out.ascent = out.ascent.max(shift + over.ascent);
        let dx = (width - over.width) / 2.0;
        out.push_child(over, dx, -shift, Slot::Over);
    }
    if let Some(under) = under {
        let shift = out.descent + gap + under.ascent;
        out.descent = out.descent.max(shift + under.descent);
        let dx = (width - under.width) / 2.0;
        out.push_child(under, dx, shift, Slot::Under);
    }
    out
}

/// `\overbrace` and `\underbrace`: a horizontal brace stretched across the
/// body, clearing it by the bar gap.
fn layout_horizontal_brace(
    cx: &Cx<'_>,
    body_node: NodeId,
    over: bool,
    span: Span,
    size: f32,
    style: Style,
    diagnostics: &mut Vec<Diagnostic>,
) -> MathBox {
    let em = style.em(cx.font, size);
    let constants = cx.font.constants();
    let body = layout_node(cx, body_node, size, style, diagnostics);
    let symbol = if over { '⏞' } else { '⏟' };
    let mut width = body.width.max(0.5 * em);
    let gap = constants.overbar_vertical_gap * em;

    let (stretched, brace_width, brace_ascent, brace_descent) = match cx.font.glyph(symbol) {
        Some(glyph) => {
            let stretched = cx.font.stretch_horizontal(glyph, width / em);
            let (brace_width, ascent, descent) = horizontal_metrics(cx, &stretched, em);
            width = width.max(brace_width);
            (Some(stretched), brace_width, ascent, descent)
        }
        None => {
            diagnostics.push(Diagnostic::new(
                span,
                format!("font has no glyph for U+{:04X}", symbol as u32),
            ));
            (None, 0.0, 0.0, 0.0)
        }
    };

    let mut out = MathBox {
        width,
        ..MathBox::empty()
    };
    let body_dx = (width - body.width) / 2.0;
    if over {
        let shift = body.ascent + gap + brace_descent;
        out.ascent = (shift + brace_ascent).max(body.ascent);
        out.descent = body.descent;
        if let Some(stretched) = &stretched {
            emit_stretched_horizontal(
                stretched,
                (width - brace_width) / 2.0,
                -shift,
                em,
                &mut out.items,
            );
        }
        out.push_child(body, body_dx, 0.0, Slot::Body);
    } else {
        let shift = body.descent + gap + brace_ascent;
        out.ascent = body.ascent;
        out.descent = (shift + brace_descent).max(body.descent);
        if let Some(stretched) = &stretched {
            emit_stretched_horizontal(
                stretched,
                (width - brace_width) / 2.0,
                shift,
                em,
                &mut out.items,
            );
        }
        out.push_child(body, body_dx, 0.0, Slot::Body);
    }
    out
}

/// Appendix G Rules 5–6: a Bin becomes Ord at a row edge, after Bin/Op/Rel/
/// Open/Punct, or before Rel/Close/Punct.
fn cancel_binary(classes: &mut [AtomClass]) {
    for index in 0..classes.len() {
        if classes[index] != AtomClass::Bin {
            continue;
        }
        let left_allows = match index.checked_sub(1).map(|previous| classes[previous]) {
            Some(
                AtomClass::Bin
                | AtomClass::Op
                | AtomClass::Rel
                | AtomClass::Open
                | AtomClass::Punct,
            ) => false,
            Some(_) => true,
            None => false,
        };
        let right_allows = match classes.get(index + 1) {
            Some(AtomClass::Rel | AtomClass::Close | AtomClass::Punct) | None => false,
            Some(_) => true,
        };
        if !(left_allows && right_allows) {
            classes[index] = AtomClass::Ord;
        }
    }
}
