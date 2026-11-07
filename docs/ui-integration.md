# The UI's LaTeX pipeline

`crabsnap-ng`'s formula path now runs on this stack. `ui-math` parses LaTeX with
`math-syntax`, lays it out with `math-core` over a `math-font` face, and converts
the result to `ui_core::Formula`; the MathML stage and `formulary` are gone.
The public seam did not move, so no widget, Markdown or engine-context code
changed in the swap.

## Where the seam sits

- `ui/crates/ui-core/src/math.rs:51` — `Formula` (width, ascent, descent, items,
  errors) and `ui-core/src/math.rs:114` `MathItem` (glyph, rule, background,
  line) are the contract above the seam.
- `ui/crates/ui-platform/src/text_pass.rs:500` — `TextPass::layout_math`
  implements the engine's layout trait, caching by (fonts version, family,
  source, size, display, error colours). `text_pass.rs:589` turns a formula
  into paint primitives, `text_pass.rs:129` places a stroked line.
- Faces are resolved by family, admitted to the rasterizer, and their bytes are
  leaked once so the borrowed face outlives the pass.
- Callers are unchanged: `ui_widgets::Math`, Markdown's `MathEditing`,
  inline objects in `ui-richtext`, and the wrappers in `ui-widgets-md3`,
  `ui-widgets-fluent` and `ui-widgets-glass`.

## The path inside `ui-math`

`ui/crates/ui-math/src/math.rs` is the whole adapter:

- `MathFace::new` (line 58) parses the bytes with `math_font::Face` and refuses
  a face without a MATH table, so the error is reported once per face.
- `layout` (line 67) is the strict path: any parse or layout diagnostic is
  `Error::Latex`.
- `layout_recovering` (line 93) keeps the structure around refused runs. Every
  diagnostic whose kind is not `Incomplete` becomes a `MathErrorMark`, and its
  rectangle is the smallest laid-out node covering the diagnostic's span
  (`mark_rect`), not just an error node the parser may or may not have made.
  `mark_items` draws the optional panel and a hairline border in the formula's
  own ink when no pack style is given.
- `probe` (line 259) reads the table directory only, so a font list can be
  scanned without laying anything out.

## Mapping

| `ui-math` item | `math_core::Item` | Note |
| --- | --- | --- |
| `Glyph` | `Glyph` | glyph id is the raw OpenType index; `advance` is 0 and `mirrored` false |
| `Rule` | `Rule` | `y` is the top edge in both |
| `Background` | `Background` | put before glyphs, rules and lines |
| `Line` | `Line` | endpoints and thickness, `color` optional |
| `Formula.errors` | diagnostics + `Geometry` | `Incomplete` is not a mark; every other kind is |

Glyph ids belong to the face the painter rasterizes because both sides use the
same bytes. This was checked on the system face the tests select
(`a_fraction_adds_depth_and_a_bar` finds exactly two glyphs at script size).

## The line primitive

`\cancel` used to be dropped at the UI boundary. The engine now draws lines:
`ui_core::math::MathItem::Line` (`ui-core/src/math.rs:138`),
`ui-core::draw::LinePrim` (`draw.rs:221`), `PrimKind::Line` (`draw.rs:781`),
and `Primitives::push_line` (`draw.rs:1181`). `text_pass` maps both formula and
inline math lines from formula space to the widget's box. The CPU rasterizer
covers the segment with a signed-distance stroke (`ui-paint/src/lib.rs:591`);
the GPU backend uses a per-kind program and two shaders
(`ui-paint-gpu/src/shaders/line.{vert,frag}.glsl`,
`ui-paint-gpu/src/renderer.rs:2120`). `gpu_scene` draws a diagonal and checks
the two backends agree; the largest difference is 1 LSB.

## Verification

Run in `ui/` unless noted:

| Command | Result |
| --- | --- |
| `cargo test -p ui-math` | 5 tests green, including `a_cancel_draws_its_diagonal` |
| `cargo test --workspace --no-fail-fast` | exit 0 |
| nested release `gpu_scene` | every invariant holds; 3 draw calls (rects, line, glyphs) |
| release gallery, math pages | formulas render; `\cancel`/`\bcancel`/`\xcancel` strokes visible in a screenshot |

The four original `ui-math` tests ran unchanged and passed: they assert
structure (a fraction is taller, scripts are smaller, a mistyped command keeps
the fraction, an unclosed group is repaired, a text face is refused), not
exact metrics. The new test pins the diagonal's existence and inherited ink.

Parse plus layout costs 0.9–3.2 µs per formula on DejaVu Math TeX Gyre (row,
scripts, fraction, root, integral, matrix; 2000 rounds in a temporary release
example). For the same shapes `formulary` costs 0.8–2.1 µs starting from
MathML, so this stack is 1.2–1.9× slower and pays no LaTeX→MathML stage in
exchange. Both figures are far below a frame budget, and the text pass caches
by source anyway.

## Remaining gaps

- Structural editing is not wired to the UI yet (7.5), and neither are the
  Markdown source/popover modes (7.6) or the editable-container entries (7.7).
  The pipeline replacement is complete without them.
- The editor cannot yet reach environments that take their own arguments
  (`array`, `matrix*`, `alignat`), and an environment name or `\hspace` length
  cannot be edited after insertion. `\pmod` exports as its expansion, so its
  source spelling is not preserved.
- `mirrored` is always false: mathstack has no RTL input. The paint path
  already refused mirrored items, so nothing regressed.
- There is no rollback branch. The `crabsnap-ng` checkout is not a git
  repository, so the previous `math.rs` was replaced in place. Its dependencies
  (`formulary 0.3.0`, `pulldown-latex 0.8.0`) are named here, but restoring the
  old path means rewriting the adapter, not checking out a branch.
- The full gallery self-test still reports failures in the Markdown page
  (task-list boxes and two caret-column checks). They concern Markdown cell
  geometry, not formula layout, and are `[unverified]` as to whether they
  predate the swap.

## Corrections

**"Nothing in the UI repository has been changed" (7.1).** That was true of the
investigation, not of the plan. 7.2c added the line primitive and 7.2d replaced
the adapter; the line primitive landed first so the adapter never had to drop a
`Line` item.

**"The smallest boundary is inside `ui-math`" held.** No caller changed. The
only UI-wide additions were the line primitive and its paint paths, which the
formula contract needed regardless of which layout engine produced the items.

## How each claim was checked

- File and line references were read from the working trees on 2026-11-07.
- The contract and callers come from a workspace search for
  `ui_math`/`math_layout`; the test and build results are the commands in the
  table above, run on the same date.
- The recovery behaviour was reproduced with `\frac{1}{1+` (no mark) and
  `\frac{1}{1+\frc{1}{n}}` (one mark in the denominator); `ui-math`'s tests
  assert both.
