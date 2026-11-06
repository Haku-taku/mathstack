# M4: the editing model — identity and caret geometry

The formula the engine lays out is now also the formula an editor addresses.
This milestone adds stable node ids, named slots, caret paths, hit testing and
selection. It does not yet add structural edit operations or the IME
composition node; those belong to the next milestone and are listed at the end.

## Arena and identity

`math-core::Model` is an arena: `alloc(kind, span)` returns a `NodeId`, and
`node`, `kind`, `span` read it back. `Model::root` names the root. The parser
writes through `ModelBuilder`, which fixes the root at the end so a `Model`
always has one; a parsed document therefore has one representation:

```rust
pub struct Document {
    pub source: String,
    pub model: Model,
    pub diagnostics: Vec<Diagnostic>,
}
```

Children are `NodeId`s stored in the variant that owns them:

```rust
Fraction { numerator: NodeId, denominator: NodeId, bar: bool, forced: Option<bool> }
Scripts { base: NodeId, sup: Option<NodeId>, sub: Option<NodeId> }
```

`NodeKind::children()` returns every child with the `Slot` it occupies
(`Body`, `Base`, `Numerator`, `Denominator`, `Superscript`, `Subscript`,
`Degree`, `Over`, `Under`). The slot enum is what caret paths name; there is no
separate slot array on the node. Typing the children per variant means an
invalid slot combination cannot be built and every traversal is exhaustive,
while `children()` still gives a uniform walk.

Every node keeps its `Span` and an `Origin` (`Source`, `Macro { definition,
call }` or `Synthetic`). `Origin` is reserved: macro expansion does not fill it
yet, so every parsed node currently reads as `Source`. Filling it is the point
at which expanded content can be marked read-only (see the design's macro
section); it is not required by caret geometry.

Ids are indices into the arena, so they are stable for as long as the node
lives. A reparse builds a new arena and new ids, which is the documented
behaviour for an external source change. The arena has no free list yet;
structural edits and deletions arrive with the editing operations.

## Caret paths

```rust
pub struct Step { pub node: NodeId, pub slot: Slot, pub index: usize }
pub struct CaretPath { pub steps: Vec<Step> }
```

The first step names the root. Each following step selects a child of the
previous step's node; `index` counts the children of that slot that carry a
node (decorations without an id, such as `\left…\right` fences, do not take a
position). The last step is the caret itself: it is the boundary before child
`index`, or after the final child when `index == len`. Because a fraction's
numerator and denominator are different slots, their carets differ even though
both slots hold one row.

`Geometry::caret(path)` walks the steps, rejecting a step whose node is not the
current container or whose index is out of range, then returns the boundary
rectangle. The rectangle's x is the next child's left edge, or the previous
child's right edge at the end of a slot. Its vertical extent is the union of
the slot's child boxes; an empty slot has no ink to size the caret, so it gets
a one-em minimum height around the slot's baseline, clamped to the container so
the caret never leaves the formula.

Paths are plain data (ids and indices), so a host can serialize or cache them.
`Geometry::carets()` yields every caret the layout offers in reading order,
which is what an invariant test walks.

## Box tree and geometry index

Layout used to flatten children into a single item list as it composed boxes.
`MathBox` now keeps its positioned children (`Placed { dx, dy, slot, child }`)
and flattens only at the end, children before their parent's own ink, so an
accent mark still paints over its base. Each box records the `NodeId` it came
from.

`Geometry::new` walks that box tree, converting every box to formula
coordinates. It records `NodeId → Rect`, keeps the tree for hit testing, and
builds a linear sequence of stops and ink rectangles in reading order:

- every slot contributes a stop before, between and after its children;
- a child that has children is entered recursively (its own stops follow the
  step that selected it);
- a leaf contributes an ink rectangle.

The sequence is what selection uses: two carets look up their stop positions,
and every ink rectangle between them is collected and merged per line.

## Hit test

`Geometry::hit_test(point)` descends the tree. A multi-slot node chooses the
slot from the point's y and its own geometry: a fraction splits at the
baseline, scripts use the base box's top and bottom, over/under scripts use the
same edges. A single-slot node uses that slot. Inside a slot the point's x is
compared with each child's midpoint, with ties going to the following child.
The search recurses while the chosen child contains the point and has children
of its own; otherwise the caret boundary at that level is returned.

## Selection

`Geometry::selection(anchor, focus)` returns rectangles covering the ink
between the two carets. Ink that shares a line is unioned, so a selection
across a fraction yields one rectangle for the numerator's line and one for the
denominator's. The calculation never consults painted pixels, only the box
tree.

## What was not here yet

The items below arrived in M5 and are described in
[`m5-editing.md`](m5-editing.md); table cells and their two-dimensional hit
testing are in [`m5-tables.md`](m5-tables.md). Nothing in this milestone
depended on them.

- Structural edits (insert, delete, replace) and arena reuse.
- `Origin::Macro` population and the read-only rule for expanded content.
- The IME `Composition` node and its commit contract.

## Verification

- `every_caret_path_resolves_to_its_own_rectangle` — every caret generated in
  reading order resolves back to the same rectangle, lies inside the formula's
  width and has positive height.
- `empty_slots_still_return_caret_rectangles` — `\frac{}{b}`'s empty numerator
  has a caret with height inside the fraction box, above the denominator's.
- `hit_test_returns_a_resolvable_caret` — a grid of points across the formula
  returns paths that resolve, and a point above a fraction bar lands in the
  numerator slot.
- `selection_covers_the_boxes_between_the_carets` — one line for a row, and the
  exact width of the single selected atom.
- `a_selection_across_a_fraction_has_one_rectangle_per_line` — numerator and
  denominator select as two rectangles, top to bottom.
- The existing layout tests kept their expected numbers through the arena
  port, and the formulary differential prints the same extents, which is the
  check that identity and geometry changed no displayed metric.

## Corrections

- **Children stay typed, not a generic slot array.** The target model sketched
  `Node { kind, span, origin, slots }` with `NodeKind` carrying no payload. In
  practice every kind has a fixed, small set of children, and a slot array
  would make "a fraction with a superscript" representable and push slot
  validation into every reader. Children are therefore typed ids in the
  variant; `Slot` survives as the name a caret path uses, and
  `NodeKind::children()` provides the uniform walk when a generic traversal is
  wanted.
- **An empty caret must not leave the formula.** The first minimum-height rule
  centred one em on the slot's baseline, so the empty numerator of a tall
  fraction produced a caret starting above the formula (the invariant test
  reported y = −0.8 for a formula whose top is 0). The extent is now clamped to
  the container box. This is a real correction to the caret rule, not a test
  tolerance: a host would otherwise draw the caret outside its own bounds.
