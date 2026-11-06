# The host editor

The engine is usable headlessly, but the editing contract only means something
under real input. `math-editor` is the host-facing layer: it owns the model,
layout, caret, selection, keystroke semantics and undo, while painting, event
translation and the text shaper stay with the host. `math-editor-demo` is a
native host that proves the contract in a window.

## Contract

```rust
impl Editor {
    fn new(source, font, size, style) -> Self;
    fn key(&mut self, key: Key, shift: bool, font) -> bool;
    fn click(&mut self, point: Point, extend: bool) -> bool;
    fn insert_latex(&mut self, source, font) -> bool;
    fn insert_superscript/subscript(&mut self, font) -> bool;
    fn select_all(&mut self) -> bool;
    fn selection_latex(&self) -> Option<String>;
    fn cut_selection(&mut self, font) -> Option<String>;
    fn source_span(&self, node) -> Option<Span>;
    fn placeholders(&self) -> Vec<(NodeId, Rect)>;
    fn set_source(&mut self, source, font);
    fn set_shaper(&mut self, shaper: Arc<dyn TextShaper>, font);
    fn set_preedit(&mut self, text, font) -> bool;
    fn commit_preedit(&mut self, text, font) -> bool;
    fn undo/redo(&mut self, font) -> bool;
}
```

`key` is the whole keyboard surface: characters, Backspace/Delete, arrows,
Home/End and Escape, with Shift as the selection modifier. `key` and
`insert_latex` return whether the display changed. The host asks the editor for
`layout()`, `geometry()`, `selection()`, `caret_rect()` and `source()`; it never
walks the box tree itself.

Structure is the document. Typing inserts nodes, Backspace removes a leaf (or
clears a required slot with an empty row), and `source()` writes the model back
to LaTeX. Editing the source text is a separate external change: `set_source`
reparses, drops history and keeps the caret at the same reading position.

## Typing and commands

A printable symbol becomes an `Atom` at the caret. `^` and `_` wrap the
preceding child in `Scripts` and put the caret in the new slot; typing into an
existing script extends it. `\` starts a command buffer, letters extend it, and
the next non-letter resolves it. The buffer is a `Composition` node at the
caret, so it is laid out and takes space like text while it is being typed, but
the writer ignores it: `source()` never contains it, and one undo removes the
resolved insertion, not the buffer (the snapshot is taken before it appears).
Escape or backspace-to-empty restores that snapshot.
Letters that arrive through the IME, as a preedit update or a commit, extend
the same buffer; an input method that composes ordinary letters therefore
cannot cancel a command that is being typed.

- a template command such as `\frac`, `\sqrt`, `\overline` or `\xrightarrow`
  is parsed as a snippet with empty arguments, so the parser remains the only
  place that knows the grammar;
- any other command is parsed as a zero-argument command (`\alpha`, `\sum`);
- an unknown command prints a message and inserts nothing.

The snippet is copied across models with `Model::copy_subtree`, which rebuilds
every id and marks the result `Origin::Synthetic`. `Model::insert_existing`
splices the copied nodes into the target row. The caret goes to the first empty
slot the snippet created, so `\frac` followed by `x` types the numerator.

`insert_latex` is the same path for a host toolbar, without the command buffer.

Commands whose next token is a delimiter consume the key that ends the
buffer instead of parsing a bare command:

- `\left(` inserts `\left(\right.` and puts the caret in the body. A later
  `\right` inside that body takes its fence key, replaces the invisible right
  fence, and moves the caret past the delimiter.
- `\middle|` inserts a `Middle` only when the caret is inside a `\left` body.
- the `\big` family (`\bigl(`) and `\not=` keep the symbol key that ended
  the buffer.

Whitespace between a delimiter command and its fence is ignored, as in LaTeX.
A `\right` or `\middle` without an enclosing `\left` removes the buffer and
reports it. Commands whose argument is text or a name — font commands such as
`\mathbb`, `\text`, `\operatorname`, `\begin`, `\hspace`, `\genfrac`,
`\substack` and `\pmod` — still reject keystroke entry and restore the
snapshot, because the model has no editable node for that argument.

### Rows the editor creates

The parser unwraps singleton brace groups, so `x^2` keeps an atom as its
superscript. The editor needs a row to type into, so it may wrap a leaf in one.
A row left holding a single child in a one-child slot is an editor artifact;
writing it would add a brace layer (`x^{{2}}`). After an insertion or a
deletion the editor collapses such a row, keeping the child and the caret, and
the writer also refuses the extra pair for a synthetic row. A delimiter body is
kept as a row because `\middle` is laid out only there. A row with two children
stays, because the source genuinely needs a group.

### Empty slots

An empty required slot is a `Row([])`, which measures zero in a static layout.
The editing layout (`layout_with_geometry`) instead reserves a 0.6 em cell for
each one, so the fraction rule, radical bar and surrounding gaps take the room
the slot will need. `Geometry::empty_slots` lists those cells and
`Editor::placeholders` returns them as rectangles; the host only paints them.
A static `layout` still measures an empty fraction as zero.

## IME

`set_preedit` inserts a `Composition` node at the caret and replaces its text as
the IME updates. The node lays out like text and the writer ignores it.
`commit_preedit` removes the composition, pushes the snapshot taken before it
appeared, and inserts the parsed commit as math in one edit. Undo therefore
removes the committed content and never reveals a preedit. `cancel_preedit`
restores that snapshot, so a dismissed composition leaves no trace and no
history entry.

While a command buffer is open, IME text made only of command letters extends
that buffer instead of becoming a preedit, and a commit of such letters does
the same. Other IME text cancels the command and starts an ordinary
composition.

## Text shaping

`\text`, `\operatorname`, error text, the IME preedit and the pending command
are measured by a host `TextShaper`. The core fallback, `FontShaper`, reads one glyph per
character from the same face; `Editor::set_shaper` installs a host
implementation and relayouts. The shaper returns glyph positions, advance and
line extents in em, so fallback fonts and combining marks stay a host concern.
A character the shaper cannot render is returned in `ShapedRun::missing`, and
layout reports one diagnostic at the construct's span.

## Source spans

The editor's source is its export. Every layout writes the model through
`to_latex_with_spans_and_spellings` and sets each node's span to its byte range
in that text,
so `source_span(node)` slices `source()` for live nodes, synthetic ones
included, and layout diagnostics carry offsets a host can underline. The
writer records a node's span around everything it emits, including the
delimiters and environment wrapper its own arm adds.

Parse diagnostics from `set_source` are the exception: their spans refer to the
text the host passed in, because that text may not equal the normalized export.

The writer normally canonicalizes symbols (`\pm` and `±` are the same atom).
The editor keeps the parser's `Spellings` beside the model, so a command the
user typed survives an export: `\pm` stays `\pm`, while a symbol typed
directly stays that symbol. Copies carry the same table, undo snapshots include
it, and deleting a subtree drops its entries. A node the editor synthesizes has
no spelling and exports canonically. Whitespace and redundant braces are still
normalized; only tokens that name the same semantic node are preserved.

## Undo and selection

Each mutating keystroke pushes a full-model snapshot; the limit is 256. Redo is
cleared by a new edit. Selection is an anchor plus the focus caret; typing or
Backspace over a selection deletes the range and inserts in one undo step.
Backward deletion walks the caret reading order: it removes the leaf before the
caret, entering a preceding group instead of deleting the group as a whole.
Forward deletion is the mirror image.

`selection_latex` exports the nodes wholly inside the selection, recursing into
a structure the range cuts; `cut_selection` exports the same text and deletes
the range in one undo step. Paste is `insert_latex`, so clipboard content is
LaTeX and crosses documents and applications.

## The native demo

`math-editor-demo` runs an eframe window:

- the display list becomes SVG through `math-svg`;
- `resvg` rasterises it with the window's pixel scale;
- egui uploads the result as a texture and draws selection rectangles and the
  caret from the geometry index;
- keyboard and IME events are translated into `Key` and `set_preedit` /
  `commit_preedit` calls; Ctrl+C/X/V move LaTeX through the system clipboard;
- input is handled before the texture is rebuilt, so an edit is painted in the
  same frame; empty slots and the pending command are laid out, not overlaid;
- while the canvas has focus it claims arrows, Tab and Escape from egui's focus
  navigation, so those keys cannot move the keyboard focus to another widget;
- a source field reparses on Apply, and a toolbar inserts fractions, radicals,
  sums and integrals.

```sh
cargo run --release -p math-editor-demo -- \
  /usr/share/fonts/wps-office/DejaVuMathTeXGyre.ttf \
  '\frac{-b\pm\sqrt{b^2-4ac}}{2a}'
```

The formula is focused on the first frame; clicking moves the caret and
dragging selects. The status row shows the exported LaTeX, the pending command,
the preedit and the diagnostic count.

## Verification

The headless tests in `crates/math-editor/tests/editor.rs` cover the branches a
host depends on:

- `typing_inserts_at_the_caret_and_arrows_move_it` — text and caret movement;
- `backspace_and_delete_remove_adjacent_leaves` — both deletion directions;
- `a_fraction_command_puts_the_caret_in_the_numerator` — command templates,
  cross-model copy and the empty-slot caret;
- `a_script_shortcut_wraps_the_previous_symbol` — script construction;
- `a_selection_is_replaced_in_one_undo_step` — selection deletion and history;
- `select_all_then_typing_replaces_the_whole_formula` — a range that covers
  whole structures removes them instead of stopping inside a required slot;
- `a_cut_selection_round_trips_through_the_clipboard` — selection export,
  cut, and reinsert through the parser;
- `exported_spans_cover_the_text_each_node_wrote` — writer offsets slice the
  emitted atom and fraction text;
- `synthetic_nodes_get_spans_in_the_exported_source` — a typed node's span
  follows it when text is inserted before it;
- `empty_required_slots_get_placeholders` — empty numerator and denominator
  reserve boxes, a filled fraction reserves none;
- `a_pending_command_takes_space_without_entering_the_source` — the buffer
  widens the layout, stays out of `source()`, and never enters undo;
- `ime_letters_extend_a_pending_command` — preedit and commit letters continue
  a command after `\` instead of cancelling it;
- `a_typed_command_keeps_its_spelling` — the editor exports the command the
  user typed rather than the symbol's canonical character;
- `left_and_right_fences_edit_a_delimiter` — `\left(` opens a delimiter, typing
  fills its body, and `\right)` closes it and moves the caret past it;
- `middle_fence_keeps_the_caret_inside_the_delimiter` — the body's caret stops
  survive the specially laid-out `\middle` row, so typing and `\right)` work;
- `deleting_from_a_typed_group_does_not_add_braces` — shrinking `x^{ab}` to one
  child exports `x^{a}`, not `x^{{a}}`;
- `commands_taking_a_symbol_keep_the_key_that_ended_them` — `\bigl(` and
  `\not=` insert their structure instead of restoring the snapshot;
- `ime_preedit_is_not_written_and_commits_as_math` — composition export,
  commit, one undo step, and no history entry for the preedit itself;
- `text_layout_uses_the_host_shaper` — a host run overrides the face fallback
  for width, ascent, descent, glyph position and missing-glyph diagnostics;
- `the_editor_relayouts_with_an_installed_shaper` — `set_shaper` reaches the
  editor's layout path;
- `editing_a_table_cell_uses_the_hit_tested_row` — click to cell to typing;
- `backspace_removes_an_empty_script_slot_and_then_the_script` — an empty
  optional slot is removed as a unit, then its wrapper;
- `copying_a_parsed_subtree_preserves_its_structure` — a broad formula
  round-trips after every node is copied; dropping one child in
  `NodeKind::map_children` makes it fail;
- `undo_and_redo_walk_the_edit_history`;
- `an_unknown_command_reports_and_inserts_nothing`.

The native window was rendered in a nested Hyprland session with the DejaVu
Math TeX Gyre face; typing characters and Backspace through the compositor
updated both the formula and the source row. A second pass opened
`a+\frac{}{}`, saw the rule and two placeholder cells take space, typed a
command and watched its text occupy the layout, then resolved it and typed a
numerator: the new fraction appeared in the same event, without waiting for
another key. This is a manual check, not a golden-image gate.

## Corrections

- **An empty formula had no caret at all.** `Geometry` derived slots from
  placed boxes, so a row with no children had no slot and no caret; the editor
  had nowhere to insert its first symbol. An empty row now gets a `Body` slot,
  and its one-em caret is clamped to the nearest non-empty ancestor instead of
  to its own zero-height box. The M4 invariant test caught the first attempt
  (a caret starting at y = −0.8) before the root case was exercised.
- **Hit testing did not descend into the right half of a lone child.** The
  chosen boundary index was also used as the child index, so a point right of
  a table's midpoint produced a caret after the table instead of inside its
  cell. When the containing child is on the other side of the boundary, the
  path now records that child's index; the table-cell editing test pins it.
- **Backspace deleted the child before the previous boundary.** The first
  implementation read the previous stop's index, which names the child *before*
  that boundary; `ab` at the end therefore deleted `a`. Deletion now starts
  from the current caret's boundary and removes child `index - 1`.
- **Editor-created rows exported extra braces.** Hand-built `Row([atom])`
  arguments are written as `{{atom}}` so a hand-built row survives a round
  trip. Collapsing the singleton row after insertion keeps the editor's export
  identical to what a user would type.
- **A freed denominator stayed referenced by its fraction.** The parser
  unwraps a singleton brace group by returning its child and abandoning the
  row, but the builder marked that row alive. `Model::remove` locates a
  parent by scanning live nodes, found the abandoned row first, and detached
  the denominator from it while the fraction still pointed at the freed node.
  `ModelBuilder::build` now marks only nodes reachable from the root alive and
  puts the rest on the free list, so an arena invariant holds for parsed and
  hand-built models alike.
- **Deleting inside a group produced `{{a}}`.** A deletion left the row that
  had held the removed sibling with one child, and the writer doubled the
  braces of every one-item argument row to keep its structure; an editor
  container is not a group, so the export grew a layer. Deletion now collapses
  redundant one-child rows in non-list slots (mapping the caret through the
  collapsed row), and the writer only doubles a source row, not a synthetic
  one. Pinned by shrinking a typed `x^{ab}`.
- **Arrow keys moved keyboard focus to another widget.** The canvas requested
  focus with egui's default `EventFilter`, so egui treated arrows, Tab and
  Escape as focus navigation while the editor also handled them; focus jumped
  to the source field or toolbar and the formula stopped receiving keys. The
  demo now locks those four events to the canvas while it has focus.
- **The demo painted one event behind.** It rebuilt the texture before
  handling input and never requested a repaint, so the last edit stayed
  invisible until an unrelated event; a resolved `\frac` therefore looked like
  nothing had happened. Input now runs before the texture is built, with the
  focus flag saved from the previous frame.
- **Placeholders and the command buffer floated over the layout.** Both were
  host-drawn overlays: an empty slot measured zero, so a placeholder could
  cover its neighbours, and the command buffer existed only in the status row.
  The editing layout now reserves a 0.6 em cell for every empty row, and a
  pending command is a `Composition` node; both are laid out like content.
  A static `layout` and the writer are unchanged, so rendered formulas and
  `source()` never see either.
- **An IME letter cancelled a pending command.** A host may deliver ordinary
  letters as IME preedit updates; each update cancelled the command buffer, so
  typing `\` then `f` removed the backslash and left a plain `f`. Letters that
  can belong to a command now extend an open buffer from both the preedit and
  commit paths, and only other text cancels it. The regression test drives the
  preedit and commit paths headlessly.
- **`\left(` disappeared instead of opening a delimiter.** The command buffer
  resolved `left` on the `(` and parsed the bare command, which needs a
  `\right` and fails; the editor then restored the pre-command snapshot and
  reported an unknown command. `\left` now consumes the key that ends its
  buffer as the fence and inserts a `Delimited` with an invisible right fence;
  typing `\right)` inside sets that fence and leaves the delimiter. The test
  also covers the space LaTeX allows between the command and its fence.
- **Export silently rewrote a command the user typed.** The model stores
  semantics, so `\pm` and `±` are the same atom, and the writer emitted the
  Unicode character for both; a typed `\le` likewise became `≤`. A `Spellings`
  side table now carries the parser's token by node id, and export emits it
  while the node still has the shape the token described. The table travels
  through copy, paste and undo, and a deleted subtree drops its entries;
  synthetic and macro-expanded nodes fall back to the canonical export. The
  parser test pins the split between `Document::to_latex` and the canonical
  `to_latex`.
- **Selection deletion stopped inside a required slot.** Deleting a range
  walked backwards one leaf at a time, so a selected fraction was emptied and
  left behind; the replacement was then inserted before it. Range deletion now
  removes every node whose stop interval lies inside the selection and only
  descends into a node the range cuts. It also relayouts after a
  selection-only Backspace, which previously left stale geometry.

## Not built yet

- Command arguments beyond the template list; a full command palette and
  argument placeholders. Keystroke entry does not cover name or text arguments
  (`\mathbb`, `\text`, `\operatorname`, `\begin`, `\hspace`, `\genfrac`,
  `\substack`, `\pmod`), command fences (`\left\langle`), or optional
  brackets (`\sqrt[`, `\smash[`, `\xrightarrow[`); the buffer currently
  rejects them or consumes the bracket.
- Incremental layout; the editor relayouts the whole formula per edit, which
  the M5 budget measurements justify at this scale.
- Browser hosts and the visual golden-image gate. The native window is the
  first L4 renderer, not the gate.
