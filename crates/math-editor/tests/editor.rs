use math_core::{
    Constants, GlyphId, InkBounds, MathFont, Model, NodeId, NodeKind, Origin, Point, ShapedRun,
    Style, TextShaper, TextStyle,
};
use math_editor::{Editor, Key};
use math_syntax::{parse, to_latex};
use std::sync::Arc;

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
        Constants::default()
    }
}

fn editor(source: &str) -> Editor {
    Editor::new(source, &FakeFont, 16.0, Style::TEXT)
}

fn press(editor: &mut Editor, key: Key) -> bool {
    editor.key(key, false, &FakeFont)
}

fn press_shift(editor: &mut Editor, key: Key) -> bool {
    editor.key(key, true, &FakeFont)
}

fn press_command(editor: &mut Editor, name: &str) {
    press(editor, Key::Character('\\'));
    for symbol in name.chars() {
        press(editor, Key::Character(symbol));
    }
}

#[test]
fn typing_inserts_at_the_caret_and_arrows_move_it() {
    let mut editor = editor("a");
    assert!(press(&mut editor, Key::Character('b')));
    assert_eq!(editor.source(), "ab");
    press(&mut editor, Key::Left);
    press(&mut editor, Key::Character('c'));
    assert_eq!(editor.source(), "acb");
}

#[test]
fn backspace_and_delete_remove_adjacent_leaves() {
    let mut editor = editor("ab");
    press(&mut editor, Key::Backspace);
    assert_eq!(editor.source(), "a");
    press(&mut editor, Key::Home);
    press(&mut editor, Key::Delete);
    assert_eq!(editor.source(), "");
}

#[test]
fn a_fraction_command_puts_the_caret_in_the_numerator() {
    let mut editor = editor("");
    press(&mut editor, Key::Character('\\'));
    for letter in ['f', 'r', 'a', 'c'] {
        press(&mut editor, Key::Character(letter));
    }
    press(&mut editor, Key::Character(' '));
    assert_eq!(editor.source(), "\\frac {}{}");
    press(&mut editor, Key::Character('x'));
    assert_eq!(editor.source(), "\\frac {x}{}");
}

#[test]
fn a_script_shortcut_wraps_the_previous_symbol() {
    let mut editor = editor("x");
    press(&mut editor, Key::Character('^'));
    press(&mut editor, Key::Character('2'));
    assert_eq!(editor.source(), "x^{2}");
    let root = editor.model().root();
    let NodeKind::Row(children) = editor.model().kind(root) else {
        panic!("expected a row");
    };
    assert!(matches!(
        editor.model().kind(children[0]),
        NodeKind::Scripts { .. }
    ));
}

#[test]
fn a_selection_is_replaced_in_one_undo_step() {
    let mut editor = editor("abc");
    press(&mut editor, Key::Home);
    press_shift(&mut editor, Key::Right);
    press_shift(&mut editor, Key::Right);
    assert!(!editor.selection().is_empty());
    press(&mut editor, Key::Character('x'));
    assert_eq!(editor.source(), "xc");
    assert!(editor.undo(&FakeFont));
    assert_eq!(editor.source(), "abc");
}

#[test]
fn ime_preedit_is_not_written_and_commits_as_math() {
    let mut editor = editor("a");
    assert!(editor.set_preedit("xy", &FakeFont));
    assert_eq!(editor.preedit(), Some("xy"));
    assert_eq!(editor.source(), "a");
    assert!(editor.commit_preedit("xy", &FakeFont));
    assert_eq!(editor.source(), "axy");
    assert!(editor.undo(&FakeFont));
    assert_eq!(editor.source(), "a");
    assert!(
        !editor.undo(&FakeFont),
        "the committed composition pushed exactly one undo step"
    );
}

#[test]
fn editing_a_table_cell_uses_the_hit_tested_row() {
    let mut editor = editor("\\begin{matrix}a&b\\\\c&d\\end{matrix}");
    let table = find_table(editor.model(), editor.model().root()).expect("a table");
    let rows = match editor.model().kind(table) {
        NodeKind::Table { rows, .. } => rows.clone(),
        _ => panic!("expected a table"),
    };
    let cells = match editor.model().kind(rows[0]) {
        NodeKind::TableRow { cells } => cells.clone(),
        _ => panic!("expected a row"),
    };
    let rect = editor.geometry().rect_of(cells[1]).expect("a cell box");
    let point = Point {
        x: rect.x + rect.width * 0.75,
        y: rect.y + rect.height / 2.0,
    };
    let clicked = editor.click(point, false);
    println!(
        "clicked={clicked} caret={:?} src={:?}",
        editor.caret(),
        editor.source()
    );
    press(&mut editor, Key::Character('x'));
    println!(
        "after type src={:?} msg={:?}",
        editor.source(),
        editor.message()
    );
    let cells = match editor.model().kind(rows[0]) {
        NodeKind::TableRow { cells } => cells.clone(),
        _ => panic!("expected a row"),
    };
    let entries = match editor.model().kind(cells[1]) {
        NodeKind::Row(children) => children.len(),
        _ => panic!("expected a cell row"),
    };
    assert_eq!(entries, 2, "the typed symbol landed in the second cell");
}

#[test]
fn copying_a_parsed_subtree_preserves_its_structure() {
    let source = "\\frac{a_1^2}{\\sqrt[3]{b}} + \\left( \\sum_{i=1}^{n} i^2 \\right) \
                  + \\hat{x} + \\overline{ab} + \\begin{pmatrix} 1 & 2 \\\\ 3 & 4 \\end{pmatrix}";
    let document = parse(source);
    let mut target = Model::new();
    let copied = target.copy_subtree(&document.model, document.root(), Default::default(), {
        Origin::Synthetic
    });
    target.set_root(copied);
    assert_eq!(
        to_latex(&target, target.root()),
        to_latex(&document.model, document.root())
    );
}

#[test]
fn undo_and_redo_walk_the_edit_history() {
    let mut editor = editor("a");
    press(&mut editor, Key::Character('b'));
    assert_eq!(editor.source(), "ab");
    assert!(editor.undo(&FakeFont));
    assert_eq!(editor.source(), "a");
    assert!(editor.redo(&FakeFont));
    assert_eq!(editor.source(), "ab");
    assert!(!editor.undo(&FakeFont) || editor.source() == "a");
}

#[test]
fn an_unknown_command_reports_and_inserts_nothing() {
    let mut editor = editor("a");
    press(&mut editor, Key::Character('\\'));
    for letter in ['n', 'o', 'p', 'e'] {
        press(&mut editor, Key::Character(letter));
    }
    press(&mut editor, Key::Character(' '));
    assert_eq!(editor.source(), "a");
    assert!(editor.message().is_some());
}

fn find_table(model: &Model, id: NodeId) -> Option<NodeId> {
    if matches!(model.kind(id), NodeKind::Table { .. }) {
        return Some(id);
    }
    model
        .kind(id)
        .children()
        .into_iter()
        .find_map(|child| find_table(model, child.id))
}

struct BlockShaper;

impl TextShaper for BlockShaper {
    fn shape(&self, _text: &str, _style: TextStyle) -> ShapedRun {
        ShapedRun {
            width: 2.0,
            ascent: 1.0,
            ..ShapedRun::default()
        }
    }
}

#[test]
fn the_editor_relayouts_with_an_installed_shaper() {
    let mut editor = editor("\\text{ab}");
    assert_eq!(editor.layout().width, 16.0);
    editor.set_shaper(Arc::new(BlockShaper), &FakeFont);
    assert_eq!(editor.layout().width, 32.0);
}

#[test]
fn backspace_removes_an_empty_script_slot_and_then_the_script() {
    let mut editor = editor("x^2");
    press(&mut editor, Key::End);
    assert!(press(&mut editor, Key::Backspace));
    assert_eq!(editor.source(), "x^{}");
    assert!(press(&mut editor, Key::Backspace));
    assert_eq!(editor.source(), "x");
}

#[test]
fn a_cut_selection_round_trips_through_the_clipboard() {
    let source = "a+\\frac{b}{c}";
    let document = parse(source);
    let expected = to_latex(&document.model, document.root());
    let mut editor = editor(source);
    assert!(editor.select_all());
    assert_eq!(editor.selection_latex().as_deref(), Some(expected.as_str()));
    let copied = editor.cut_selection(&FakeFont).expect("a selection");
    assert_eq!(copied, expected);
    assert_eq!(editor.source(), "");
    assert!(editor.insert_latex(&copied, &FakeFont));
    assert_eq!(editor.source(), expected);
}

#[test]
fn select_all_then_typing_replaces_the_whole_formula() {
    let mut editor = editor("a+\\frac{b}{c}");
    assert!(editor.select_all());
    assert!(!editor.selection().is_empty());
    press(&mut editor, Key::Character('x'));
    assert_eq!(editor.source(), "x");
}

#[test]
fn synthetic_nodes_get_spans_in_the_exported_source() {
    let mut editor = editor("a");
    press(&mut editor, Key::Character('b'));
    press(&mut editor, Key::Home);
    press(&mut editor, Key::Character('x'));
    assert_eq!(editor.source(), "xab");
    let root = editor.model().root();
    let NodeKind::Row(children) = editor.model().kind(root) else {
        panic!("expected a row");
    };
    let b = *children.last().expect("the typed atom");
    let span = editor.source_span(b).expect("a live node");
    assert_eq!(&editor.source()[span.start..span.end], "b");
    assert!(matches!(editor.model().node(b).origin, Origin::Synthetic));
}

#[test]
fn empty_required_slots_get_placeholders() {
    let empty = editor("\\frac{}{}");
    let boxes = empty.placeholders();
    assert_eq!(boxes.len(), 2, "numerator and denominator");
    assert!(
        boxes[0].1.bottom() < boxes[1].1.bottom(),
        "the numerator's box sits above the denominator's"
    );
    assert!(boxes[0].1.width > 0.0, "the placeholder occupies space");
    assert!(editor("\\frac{a}{b}").placeholders().is_empty());
    assert!(editor("a+b").placeholders().is_empty());
}

#[test]
fn a_pending_command_takes_space_without_entering_the_source() {
    let mut editor = editor("a");
    let before = editor.layout().width;
    press(&mut editor, Key::Character('\\'));
    for letter in ['f', 'r', 'a', 'c'] {
        press(&mut editor, Key::Character(letter));
    }
    assert!(editor.command().is_some());
    assert_eq!(editor.source(), "a", "the buffer is not document content");
    assert!(
        editor.layout().width > before,
        "the pending command reserves layout space"
    );
    press(&mut editor, Key::Character(' '));
    assert_eq!(editor.source(), "a\\frac {}{}");
    assert!(editor.undo(&FakeFont));
    assert_eq!(editor.source(), "a");
    assert!(!editor.undo(&FakeFont), "the buffer never entered history");
}

#[test]
fn ime_letters_extend_a_pending_command() {
    let mut editor = editor("");
    press(&mut editor, Key::Character('\\'));
    assert!(editor.set_preedit("f", &FakeFont));
    assert_eq!(editor.command(), Some("f"));
    assert!(
        editor.preedit().is_none(),
        "letters join the command, not a preedit"
    );
    assert!(editor.set_preedit("fr", &FakeFont));
    assert_eq!(editor.command(), Some("fr"));
    assert!(editor.commit_preedit("frac", &FakeFont));
    assert_eq!(editor.command(), Some("frac"));
    assert!(press(&mut editor, Key::Character(' ')));
    assert_eq!(editor.source(), "\\frac {}{}");
}

#[test]
fn a_typed_command_keeps_its_spelling() {
    let mut editor = editor("");
    press(&mut editor, Key::Character('\\'));
    for letter in ['p', 'm'] {
        press(&mut editor, Key::Character(letter));
    }
    press(&mut editor, Key::Character(' '));
    assert_eq!(editor.source(), "\\pm ");
}

#[test]
fn left_and_right_fences_edit_a_delimiter() {
    let mut editor = editor("");
    press(&mut editor, Key::Character('\\'));
    for letter in ['l', 'e', 'f', 't'] {
        press(&mut editor, Key::Character(letter));
    }
    // LaTeX allows a space before the fence; it must not resolve the buffer.
    press(&mut editor, Key::Character(' '));
    press(&mut editor, Key::Character('('));
    assert_eq!(editor.source(), "\\left (\\right .");
    press(&mut editor, Key::Character('a'));
    assert_eq!(editor.source(), "\\left (a\\right .");
    press(&mut editor, Key::Character('\\'));
    for letter in ['r', 'i', 'g', 'h', 't'] {
        press(&mut editor, Key::Character(letter));
    }
    press(&mut editor, Key::Character(' '));
    press(&mut editor, Key::Character(')'));
    assert_eq!(editor.source(), "\\left (a\\right )");
    press(&mut editor, Key::Character('x'));
    assert_eq!(editor.source(), "\\left (a\\right )x");
}

#[test]
fn deleting_from_a_typed_group_does_not_add_braces() {
    let mut editor = editor("");
    for symbol in "x^ab".chars() {
        press(&mut editor, Key::Character(symbol));
    }
    assert_eq!(editor.source(), "x^{ab}");
    press(&mut editor, Key::Backspace);
    assert_eq!(editor.source(), "x^{a}");
}

#[test]
fn middle_fence_keeps_the_caret_inside_the_delimiter() {
    let mut editor = editor("");
    press_command(&mut editor, "left");
    press(&mut editor, Key::Character('('));
    press(&mut editor, Key::Character('a'));
    press_command(&mut editor, "middle");
    press(&mut editor, Key::Character('|'));
    press(&mut editor, Key::Character('b'));
    press_command(&mut editor, "right");
    press(&mut editor, Key::Character(')'));
    assert_eq!(editor.source(), "\\left (a\\middle |b\\right )");
    press(&mut editor, Key::Character('x'));
    assert_eq!(editor.source(), "\\left (a\\middle |b\\right )x");
}

#[test]
fn commands_taking_a_symbol_keep_the_key_that_ended_them() {
    let mut big = editor("");
    press_command(&mut big, "bigl");
    press(&mut big, Key::Character('('));
    assert_eq!(big.source(), "\\bigl (");

    let mut not = editor("");
    press_command(&mut not, "not");
    press(&mut not, Key::Character('='));
    assert_eq!(not.source(), "\\not=");
}

#[test]
fn command_named_fences_complete_left_middle_and_right() {
    let mut editor = editor("");
    press_command(&mut editor, "left");
    press(&mut editor, Key::Character('\\'));
    for letter in "langle".chars() {
        press(&mut editor, Key::Character(letter));
    }
    press(&mut editor, Key::Character(' '));
    assert_eq!(editor.source().trim_end(), "\\left \\langle \\right .");

    press(&mut editor, Key::Character('a'));
    press_command(&mut editor, "middle");
    press(&mut editor, Key::Character('\\'));
    press(&mut editor, Key::Character('|'));
    press(&mut editor, Key::Character(' '));
    press(&mut editor, Key::Character('b'));
    press_command(&mut editor, "right");
    press(&mut editor, Key::Character('\\'));
    for letter in "rangle".chars() {
        press(&mut editor, Key::Character(letter));
    }
    press(&mut editor, Key::Character(' '));
    assert_eq!(
        editor.source().trim_end(),
        "\\left \\langle a\\middle \\| b\\right \\rangle"
    );
}

#[test]
fn not_takes_a_relation_command_name() {
    let mut editor = editor("");
    press_command(&mut editor, "not");
    press(&mut editor, Key::Character('\\'));
    for letter in "in".chars() {
        press(&mut editor, Key::Character(letter));
    }
    press(&mut editor, Key::Character(' '));
    assert_eq!(editor.source().trim_end(), "\\not\\in");
}

#[test]
fn optional_brackets_fill_the_argument_and_drop_when_empty() {
    let mut sqrt = editor("");
    press_command(&mut sqrt, "sqrt");
    press(&mut sqrt, Key::Character('['));
    assert_eq!(sqrt.source(), "");
    press(&mut sqrt, Key::Character('3'));
    press(&mut sqrt, Key::Character(']'));
    assert_eq!(sqrt.source(), "\\sqrt [3]{}");
    press(&mut sqrt, Key::Character('x'));
    assert_eq!(sqrt.source(), "\\sqrt [3]{x}");

    let mut empty = editor("");
    press_command(&mut empty, "sqrt");
    press(&mut empty, Key::Character('['));
    press(&mut empty, Key::Character(']'));
    assert_eq!(empty.source(), "\\sqrt {}");
    press(&mut empty, Key::Character('x'));
    assert_eq!(empty.source(), "\\sqrt {x}");

    let mut smash = editor("");
    press_command(&mut smash, "smash");
    press(&mut smash, Key::Character('['));
    press(&mut smash, Key::Character('t'));
    press(&mut smash, Key::Character(']'));
    press(&mut smash, Key::Character('x'));
    assert_eq!(smash.source(), "\\smash [t]{x}");

    let mut arrow = editor("");
    press_command(&mut arrow, "xrightarrow");
    press(&mut arrow, Key::Character('['));
    press(&mut arrow, Key::Character('u'));
    press(&mut arrow, Key::Character(']'));
    press(&mut arrow, Key::Character('x'));
    assert_eq!(arrow.source(), "\\xrightarrow [u]{x}");
}

#[test]
fn empty_slots_leave_without_deleting_their_base() {
    // An empty script slot is optional: Backspace removes it, not the base.
    let mut superscript = editor("");
    press(&mut superscript, Key::Character('x'));
    press(&mut superscript, Key::Character('^'));
    assert!(press(&mut superscript, Key::Backspace));
    assert_eq!(superscript.source(), "x");
    press(&mut superscript, Key::Character('y'));
    assert_eq!(superscript.source(), "xy");

    // Delete removes the same slot and leaves the caret after the base.
    let mut forward = editor("");
    press(&mut forward, Key::Character('x'));
    press(&mut forward, Key::Character('^'));
    assert!(press(&mut forward, Key::Delete));
    assert_eq!(forward.source(), "x");
    press(&mut forward, Key::Character('y'));
    assert_eq!(forward.source(), "xy");

    // A required slot keeps its empty row; Backspace leaves it without
    // touching the fraction's other child.
    let mut fraction = editor("");
    press_command(&mut fraction, "frac");
    press(&mut fraction, Key::Character(' '));
    assert!(press(&mut fraction, Key::Backspace));
    assert_eq!(fraction.source(), "\\frac {}{}");
    assert!(!press(&mut fraction, Key::Backspace));
    assert_eq!(fraction.source(), "\\frac {}{}");

    // An empty degree is optional too; its content deletes first.
    let mut radical = editor("\\sqrt[3]{x}");
    for _ in 0..4 {
        press(&mut radical, Key::Left);
    }
    assert!(press(&mut radical, Key::Backspace));
    assert_eq!(radical.source(), "\\sqrt []{x}");
    assert!(press(&mut radical, Key::Backspace));
    assert_eq!(radical.source(), "\\sqrt {x}");
}

#[test]
fn table_cells_survive_text_deletion() {
    let mut table = editor("\\begin{matrix}a&b\\\\c&d\\end{matrix}");
    for _ in 0..4 {
        press(&mut table, Key::Backspace);
    }
    assert_eq!(table.source(), "\\begin {matrix} &  \\\\  & \\end {matrix}");
    assert!(!press(&mut table, Key::Backspace));
    assert_eq!(table.source(), "\\begin {matrix} &  \\\\  & \\end {matrix}");

    press(&mut table, Key::Home);
    for _ in 0..6 {
        press_shift(&mut table, Key::Right);
    }
    press(&mut table, Key::Backspace);
    assert_eq!(table.source(), "\\begin {matrix} &  \\\\  & \\end {matrix}");
}

#[test]
fn a_cut_environment_keeps_its_name() {
    let mut editor = editor("\\begin{dcases}a&b\\\\c&d\\end{dcases}");
    let exported = editor.source();
    assert!(editor.select_all());
    let copied = editor.cut_selection(&FakeFont).expect("a selection");
    assert_eq!(copied, exported);
    assert!(editor.insert_latex(&copied, &FakeFont));
    assert_eq!(editor.source(), exported);
}
