use math_core::{
    CaretPath, Constants, GlyphId, InkBounds, MathFont, Model, NodeId, Point, ShapedGlyph,
    ShapedRun, Slot, Step, Style, TextShaper, TextStyle, layout_with_geometry,
    layout_with_shaper_and_geometry,
};

struct TestFont;

impl MathFont for TestFont {
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

/// `a + \frac{}{b} + x^2`, with an empty numerator to exercise empty slots.
fn mixed() -> (Model, NodeId) {
    Model::build(|m| {
        let a = m.symbol('a');
        let numerator = m.row(Vec::new());
        let b = m.symbol('b');
        let denominator = m.row(vec![b]);
        let fraction = m.fraction(numerator, denominator, true);
        let x = m.symbol('x');
        let two = m.symbol('2');
        let superscript = m.row(vec![two]);
        let scripts = m.scripts(x, Some(superscript), None);
        let plus = m.symbol('+');
        m.row(vec![a, plus, fraction, scripts])
    })
}

#[test]
fn every_caret_path_resolves_to_its_own_rectangle() {
    let (model, root) = mixed();
    let (layout, geometry, diagnostics) =
        layout_with_geometry(&TestFont, &model, root, 10.0, Style::TEXT);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");

    let carets: Vec<_> = geometry.carets().cloned().collect();
    assert!(!carets.is_empty());
    for (path, rect) in &carets {
        assert_eq!(
            geometry.caret(path),
            Some(*rect),
            "caret {path:?} must resolve to the rectangle it was generated with"
        );
        assert!(
            rect.x >= -0.001 && rect.x <= layout.width + 0.001,
            "caret x {} outside the formula width {}",
            rect.x,
            layout.width
        );
        assert!(rect.bottom > rect.top, "a caret must have ink height");
        assert!(
            rect.top >= -0.001 && rect.bottom <= layout.ascent + layout.descent + 0.001,
            "caret {}..{} outside the formula height {}",
            rect.top,
            rect.bottom,
            layout.ascent + layout.descent
        );
    }
}

#[test]
fn empty_slots_still_return_caret_rectangles() {
    let (model, root) = mixed();
    let (_, geometry, _) = layout_with_geometry(&TestFont, &model, root, 10.0, Style::TEXT);

    let fraction = model.kind(root);
    let math_core::NodeKind::Row(children) = fraction else {
        panic!("expected a row");
    };
    let fraction_id = children[2];
    let fraction_rect = geometry
        .rect_of(fraction_id)
        .expect("the fraction has a box");

    let numerator = CaretPath::new(vec![
        Step {
            node: root,
            slot: Slot::Body,
            index: 2,
        },
        Step {
            node: fraction_id,
            slot: Slot::Numerator,
            index: 0,
        },
    ]);
    let denominator = CaretPath::new(vec![
        Step {
            node: root,
            slot: Slot::Body,
            index: 2,
        },
        Step {
            node: fraction_id,
            slot: Slot::Denominator,
            index: 1,
        },
    ]);
    let empty = geometry.caret(&numerator).expect("empty numerator caret");
    let filled = geometry.caret(&denominator).expect("denominator caret");
    assert!(empty.bottom > empty.top);
    assert!(
        empty.x >= fraction_rect.x - 0.001 && empty.x <= fraction_rect.right() + 0.001,
        "empty-slot caret sits inside the fraction"
    );
    assert!(
        empty.top < filled.top,
        "the numerator slot is above the denominator slot"
    );
}

#[test]
fn hit_test_returns_a_resolvable_caret() {
    let (model, root) = mixed();
    let (layout, geometry, _) = layout_with_geometry(&TestFont, &model, root, 10.0, Style::TEXT);
    let height = layout.ascent + layout.descent;

    for x in [
        0.0,
        layout.width / 3.0,
        layout.width / 2.0,
        layout.width - 0.1,
    ] {
        for y in [0.0, height / 3.0, height / 2.0, height - 0.1] {
            let path = geometry
                .hit_test(Point { x, y })
                .unwrap_or_else(|| panic!("no caret at ({x}, {y})"));
            assert!(
                geometry.caret(&path).is_some(),
                "hit test at ({x}, {y}) returned an unresolvable path"
            );
        }
    }

    let math_core::NodeKind::Row(children) = model.kind(root) else {
        panic!("expected a row");
    };
    let fraction = children[2];
    let rect = geometry.rect_of(fraction).expect("the fraction has a box");
    let path = geometry
        .hit_test(Point {
            x: rect.center_x() - 0.1,
            y: rect.y + 1.0,
        })
        .expect("a caret above the fraction bar");
    assert!(
        path.steps
            .iter()
            .any(|step| step.node == fraction && step.slot == Slot::Numerator),
        "the point above the bar lands in the numerator: {path:?}"
    );
}

#[test]
fn selection_covers_the_boxes_between_the_carets() {
    let (model, root) = Model::build(|m| {
        let a = m.symbol('a');
        let plus = m.symbol('+');
        let b = m.symbol('b');
        m.row(vec![a, plus, b])
    });
    let (layout, geometry, _) = layout_with_geometry(&TestFont, &model, root, 10.0, Style::TEXT);
    let caret = |index| {
        CaretPath::new(vec![Step {
            node: root,
            slot: Slot::Body,
            index,
        }])
    };

    let all = geometry.selection(&caret(0), &caret(3));
    assert_eq!(all.len(), 1, "a single row selects as one rectangle");
    assert!((all[0].width - layout.width).abs() < 0.001);
    assert!(all[0].height > 0.0);

    let plus = geometry.selection(&caret(1), &caret(2));
    assert_eq!(plus.len(), 1);
    assert!((plus[0].width - 5.0).abs() < 0.001, "{:?}", plus[0]);
}

#[test]
fn a_selection_across_a_fraction_has_one_rectangle_per_line() {
    let (model, root) = Model::build(|m| {
        let a = m.symbol('a');
        let b = m.symbol('b');
        let numerator = m.row(vec![a]);
        let denominator = m.row(vec![b]);
        m.fraction(numerator, denominator, true)
    });
    let (_, geometry, _) = layout_with_geometry(&TestFont, &model, root, 10.0, Style::TEXT);
    let fraction = root;
    let numerator_start = CaretPath::new(vec![Step {
        node: fraction,
        slot: Slot::Numerator,
        index: 0,
    }]);
    let denominator_end = CaretPath::new(vec![Step {
        node: fraction,
        slot: Slot::Denominator,
        index: 1,
    }]);
    let rects = geometry.selection(&numerator_start, &denominator_end);
    assert_eq!(rects.len(), 2, "numerator and denominator are two lines");
    assert!(rects[0].y < rects[1].y);
}

#[test]
fn hit_test_descends_a_table_row_then_its_cell() {
    let (model, root) = Model::build(|m| {
        let a = m.symbol('a');
        let cell_a = m.row(vec![a]);
        let b = m.symbol('b');
        let cell_b = m.row(vec![b]);
        let row0 = m.table_row(vec![cell_a, cell_b]);
        let c = m.symbol('c');
        let cell_c = m.row(vec![c]);
        let d = m.symbol('d');
        let cell_d = m.row(vec![d]);
        let row1 = m.table_row(vec![cell_c, cell_d]);
        m.table(vec![row0, row1], 2, Vec::new())
    });
    let (_, geometry, _) = layout_with_geometry(&TestFont, &model, root, 10.0, Style::TEXT);

    let math_core::NodeKind::Table { rows, .. } = model.kind(root) else {
        panic!("expected a table");
    };
    let math_core::NodeKind::TableRow { cells } = model.kind(rows[1]) else {
        panic!("expected a table row");
    };
    let cell = cells[1];
    let rect = geometry.rect_of(cell).expect("the cell has a box");
    let path = geometry
        .hit_test(Point {
            x: rect.x + 1.0,
            y: rect.y + 1.0,
        })
        .expect("a caret inside the cell");

    let first = path.steps.first().expect("the path starts at the table");
    assert_eq!((first.node, first.slot, first.index), (root, Slot::Row, 1));
    let second = path.steps.get(1).expect("the path enters the row");
    assert_eq!(
        (second.node, second.slot, second.index),
        (rows[1], Slot::Cell, 1)
    );
    assert!(
        path.steps
            .iter()
            .any(|step| step.node == cell && step.slot == Slot::Body),
        "the path ends inside the chosen cell: {path:?}"
    );
}

#[test]
fn a_text_leaf_offers_a_caret_between_every_cluster() {
    let (model, root) = Model::build(|m| m.text("ab"));
    let (_, geometry, _) = layout_with_geometry(&TestFont, &model, root, 10.0, Style::TEXT);

    let paths: Vec<&CaretPath> = geometry
        .carets()
        .filter(|(path, _)| {
            path.steps
                .first()
                .is_some_and(|step| step.node == root && step.slot == Slot::Body)
        })
        .map(|(path, _)| path)
        .collect();
    let offsets: Vec<usize> = paths
        .iter()
        .filter_map(|path| path.steps.last().map(|step| step.index))
        .collect();
    assert_eq!(offsets, vec![0, 1, 2], "one caret per byte boundary");

    let middle = geometry.caret(paths[1]).expect("the caret resolves");
    assert!((middle.x - 5.0).abs() < 0.01);
    let hit = geometry
        .hit_test(Point { x: 6.0, y: 5.0 })
        .expect("a caret inside the text");
    assert_eq!(hit.steps.last().map(|step| step.index), Some(1));

    let selected = geometry.selection(paths[0], paths[2]);
    let width: f32 = selected.iter().map(|rect| rect.width).sum();
    assert!((width - 10.0).abs() < 0.01);
}

struct LigatureShaper;

impl TextShaper for LigatureShaper {
    fn shape(&self, _text: &str, _style: TextStyle) -> ShapedRun {
        ShapedRun {
            glyphs: vec![ShapedGlyph {
                glyph: GlyphId(1),
                x: 0.0,
                y: 0.0,
                size: 1.0,
                cluster: 0,
            }],
            width: 3.0,
            ascent: 0.6,
            descent: 0.2,
            missing: Vec::new(),
        }
    }
}

#[test]
fn a_ligature_cannot_be_split_by_a_caret() {
    let (model, root) = Model::build(|m| m.text("ffi"));
    let (_, geometry, _) = layout_with_shaper_and_geometry(
        &TestFont,
        &LigatureShaper,
        &model,
        root,
        10.0,
        Style::TEXT,
    );
    let offsets: Vec<usize> = geometry
        .carets()
        .filter_map(|(path, _)| path.steps.last().map(|step| step.index))
        .collect();
    assert_eq!(offsets, vec![0, 3], "the ligature is one cluster");
}
