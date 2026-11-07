# M5, part 4: structural edits, macro origins and composition

The last M5 pieces are the ones M4 listed as absent: operations that change the
tree rather than the source, provenance for expanded macros, and the IME
preedit node.

## Structural edits

The contract stays the design's: **structure is the truth, source is an
export.** A host edits the model with these operations and writes LaTeX from
the result; changing the source text (typing into it) is an external change and
reparses, giving new ids.

```rust
impl Model {
    fn insert(parent, slot, index, kind, span) -> Result<NodeId, EditError>;
    fn remove(id) -> Result<(), EditError>;
    fn replace(id, kind) -> Result<(), EditError>;
}
```

`insert` works on the slots that hold a list: a row's body, a table's rows and
a table row's cells. Single-child slots are reached through their child: typing
into an empty numerator inserts into that numerator row, not into the fraction.
`remove` detaches the node and frees its subtree. `replace` keeps the node's id
and span, frees the subtrees the new kind no longer refers to, and keeps the
ones it does — which is what wrapping a selection in a new construct needs.

Freed ids are reused by later allocations, so a host must drop an id after
removing it. That is the documented consequence of "deletions free": the arena
does not grow without bound, and ids are stable only while a node is alive.

Required children (a fraction's numerator, a script's base, a unary node's
body) cannot be removed; the call returns `EditError::RequiredChild` rather
than leave an invalid node. Replacing that child with an empty row is the
supported way to clear it.

## Macro origins

Every token now carries an `Origin`. The lexer sets `Source`; the expander sets
`Origin::Macro { definition, call }` on every token it copies from a macro body,
using the definition command's span and the call site's span. Argument tokens
are substituted as they came, so they keep their own spans and stay editable.

The parser attaches the origin of the token that starts a construct to that
construct's outermost node (`parse_atom` and the script wrapper in
`parse_item`). Children keep their own origins, so a node can be read-only while
a substituted argument inside it is not. A host checks the origin of the node
it is about to edit: `Source` is editable, `Macro` is not, and an "expand here"
command replaces the call with the body's text.

## Composition

`NodeKind::Composition(String)` is the live preedit. It lays out exactly like
`Text` (the same text path), is created with `Origin::Synthetic`, and the
writer emits nothing for it: a preedit is not part of the document. Commit
parses the text as math and replaces the node with the result in one edit, so
undo sees one step and the preedit itself is never undoable. A composition does
not need a span; the synthetic origin is the marker.

## Text leaves

A `Text(String)` stays one model leaf; the editor edits inside it by byte
offset instead of splitting it into character nodes. `	ext` is the first
name argument with a template: the editor inserts an empty leaf and puts the
caret inside it, so text mode is reachable without typing a source snippet. Layout records the
cluster boundaries the shaper returns, so the geometry index offers carets at
`Text` offsets and the selection highlights the substring between them. A
ligature is one cluster and cannot be split; a combining mark shares its
base's cluster. Editing a range replaces the string and relayouts; if the new
string forms a different cluster, the caret snaps to the nearest boundary the
new shape offers. Backspace, Delete and IME commits move by grapheme, so a
combining sequence or emoji ZWJ stays one editing unit even when the geometry
has a stop inside it. A `Function` name uses the same text path, so
`\operatorname{...}` is edited string-wise while its `Limits` survive.

Glue is part of the row's child list rather than only its width. Geometry
indices address the model's children, so a space that was measured but not
listed shifted every caret after it by one slot; keeping the zero-height space
box in `MathBox::children` makes `\pmod{}` and any row with `\quad` editable at
the right position.

## Verification

- `inserting_and_removing_a_node_reuses_the_slot` — insertion splices at the
  index, removal detaches and frees, and the next allocation reuses the id.
  Disabling the free-list pop makes it fail.
- `replacing_a_node_keeps_the_subtrees_it_still_refers_to` — wrapping a
  fraction's numerator in a row keeps the numerator and its child alive and
  frees the denominator. Passing an empty keep-set makes it fail.
- `removing_a_required_child_is_refused` — a fraction's numerator cannot be
  removed and survives the refused call.
- `macro_expansion_marks_its_nodes_and_keeps_arguments_editable` — an expanded
  atom carries `Macro` and the call span; a substituted argument is `Source`,
  a body-produced superscript is `Macro`. Dropping the origin assignment in
  `substitute` makes it fail.
- `a_composition_lays_out_like_text` and `a_live_composition_is_not_written` —
  the preedit measures like the same text and produces no LaTeX.
- `a_text_leaf_offers_a_caret_between_every_cluster` — a text leaf exposes one
  caret per cluster boundary, hit-testing selects the nearest, and the
  selection covers the substring.
- `a_ligature_cannot_be_split_by_a_caret` — a shaper that returns one glyph for
  `ffi` yields only the two ends; replacing cluster data with per-character
  boundaries makes it fail.
- `typing_inside_a_text_leaf_edits_its_string` and
  `an_ime_composition_edits_a_text_leaf_and_commits_in_one_undo` — typing and
  IME replace the string in place; undo restores the pre-composition text.
- `one_backspace_removes_a_combined_grapheme` — a combining sequence is one
  grapheme for deletion even when geometry has two cluster stops.
- `shift_arrows_select_and_replace_inside_text` — a text-internal selection
  returns its substring and typing replaces exactly that range.

## Remaining after M5

The host adapter is now `math-editor` with a native demo; see
[`host-editor.md`](host-editor.md). Cross-model copying for a commit that reuses
a parse result is `Model::copy_subtree`, and the selection-aware command set
lives in the host layer. `TextShaper` is now wired through `math-core` and
`math-editor`; browser hosts remain open.
