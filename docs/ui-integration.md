# Replacing the UI's LaTeX pipeline

This page records where `crabsnap-ng` turns LaTeX into a display list, what
`mathstack` would have to provide to stand in, and which parts of the contract
the current stack does not yet meet. It is a 7.1 investigation: nothing in the
UI repository has been changed.

## The pipeline as it stands

`ui-math` owns both third-party libraries, so nothing above it sees their
types. Its path is LaTeX → MathML → formulary tree → `ui_core::Formula`:

- `ui/crates/ui-math/src/math.rs:70` — `MathFace::new` parses a formulary
  `MathFont` once per face; it borrows the font bytes.
- `ui/crates/ui-math/src/math.rs:75` — `layout` is the strict path: any
  `pulldown-latex` diagnostic is an `Error::Latex`.
- `ui/crates/ui-math/src/math.rs:87` — `layout_recovering` repairs missing
  closers/arguments, writes `<merror>` for refused tokens, then lays the
  MathML out. `ui/crates/ui-math/src/math.rs:485` converts formulary's items
  to `MathItem` and turns the tagged error boxes into `MathErrorMark`s.
- `ui/crates/ui-math/src/math.rs:445` — `probe` reports whether a face has a
  MATH table, without laying anything out.
- `ui/crates/ui-core/src/math.rs:51` — `Formula` (width, ascent, descent,
  items, errors) and `ui-core/src/math.rs:116` `MathItem` (glyph, rule,
  background) are the contract above the seam.
- `ui/crates/ui-core/src/cx.rs:47` — the engine context exposes `layout` as a
  trait; `ui-core/src/cx.rs:329` forwards to the host. `ui-platform`'s
  `TextPass::layout_math` implements it at
  `ui/crates/ui-platform/src/text_pass.rs:460`, caching by (fonts version,
  family, source, size, display, error colours).
- Faces are resolved by family, admitted to the rasterizer, and their bytes
  are leaked once so the borrowed `MathFace` outlives the pass
  (`text_pass.rs:522`).
- Callers are `ui_widgets::Math` (`ui-widgets/src/math.rs:100`), `MathField`
  (source line plus preview), Markdown's `MathEditing::{Source, InPlace}`
  (`ui-widgets/src/markdown.rs:84`), inline objects in `ui-richtext`, and the
  per-pack wrappers in `ui-widgets-md3`, `ui-widgets-fluent` and
  `ui-widgets-glass`.

The smallest boundary is therefore **inside `ui-math`**: keep `MathFace`,
`layout`, `layout_recovering`, `probe`, `Error` and the `ui-core` types, and
replace the LaTeX/MathML/formulary body with `math-syntax` + `math-font` +
`math-core`. No widget, Markdown or engine-context code has to change.

## Mapping

| `ui-math` contract | `mathstack` | Note |
| --- | --- | --- |
| `MathFace::new(bytes, index)` | `math_font::Face::parse(bytes, index)` | both borrow the bytes; the face must carry a MATH table |
| `probe(bytes, index)` | new `Face::has_math()` over `ttf_parser`'s table directory | currently no public predicate |
| `layout` (strict) | `math_syntax::parse` + `layout_with_geometry`; any diagnostic → `Error::Latex` | mathstack's parser recovers rather than aborting, so strictness is the adapter's decision |
| `layout_recovering` | the same parse + layout; diagnostics that only say a group is incomplete are not marks | needs a structured diagnostic kind, see below |
| `Formula.width/ascent/descent` | `Layout` of the same names | |
| `MathItem::Glyph` | `math_core::Item::Glyph` | `advance` from `Face::advance`; `color` from the item; `mirrored: false` |
| `MathItem::Rule` | `math_core::Item::Rule` (`y` is the top edge) | |
| `MathItem::Background` | none | only MathML `mathbackground` produces one today |
| `Formula.errors` | `Error` nodes' geometry (`Geometry::rect_of`) plus the diagnostic for that span | message text comes from `Diagnostic::message` |
| `MathError::NoFace` | face parsing failure | |
| `MathError::Failed` | layout failure | |

Glyph ids are raw OpenType indices in both stacks (`math_font` forwards
`ttf_parser`'s `GlyphId`), and the text pass rasterizes with the same face
bytes, so the ids belong to the face the painter will use. This is the one
assumption the adapter must test first on a real system face.

## Gaps that need a decision before 7.2

1. **Structured diagnostics.** `math_core::Diagnostic` is `{ span, message }`
   (`crates/math-core/src/layout.rs:26`). The UI distinguishes "still typing"
   from "refused token": `\frac{1}{1+` must lay out with no error mark, while
   `\frc` must produce one. mathstack currently reports both through the same
   type (`"unclosed \`{\`"` versus `"unknown command \frc"`), and matching on
   message text would be a patch. Add a kind (`Incomplete`, `UnknownCommand`,
   `Layout`, …) at the source and keep the message for the UI.
2. **Cancellation lines.** `math_core::Item::Line` is emitted by
   `\cancel`, but `ui_core::MathItem` has no line variant and the paint path
   (`text_pass.rs:550`) draws only glyphs, rules and backgrounds. Either add a
   stroked segment to `ui-core` and the rasterizer, or accept that replacing
   the pipeline drops `\cancel` (formulary does not draw it either).
3. **Colours.** `\color`/`\textcolor` now reach `Item::Glyph.color`,
   `Item::Rule.color` and `Item::Line.color` as `math_core::Color`. Both
   `MathItem::Glyph` and `MathItem::Rule` already carry `color: Option<Color>`
   (`ui-core/src/math.rs`), so the adapter maps each directly; only the
   stroked line has no UI counterpart, as the cancellation gap notes.
4. **RTL mirroring.** MathML can ask for mirrored glyphs; mathstack has no
   RTL input, so `mirrored` is always false. The paint path already refuses a
   mirrored item, so there is no regression, only a capability it never had.
5. **Error-mark geometry.** `layout_recovering` needs a rectangle for a
   refused run. mathstack has it: the `Error` node is laid out and
   `Geometry::rect_of` returns its box. The adapter must walk the model,
   not just the diagnostics, and must keep the reading order the UI uses for
   messages.

## Work plan for 7.2 and 7.3

1. In mathstack: add `DiagnosticKind` and `Face::has_math`; keep the text.
2. In `ui-math`: replace the internals, keep the public API and `Error`
   variants; dependencies become `math-core`, `math-syntax`, `math-font`
   (drop `formulary` and `pulldown-latex`, which no other crate uses).
3. Run `ui-math`'s four behavioural tests unchanged first; they assert
   structure (a fraction is taller, scripts are smaller, a mistyped command
   keeps the fraction, an unclosed group is repaired, a text face is
   refused), not exact formulary metrics.
4. Re-baseline expected extents wherever a test pins them. The m1
   differential already records where this stack and formulary choose
   different rules; 11 of 19 formulas agree within 0.01 pt.
5. Remove the old path and update `docs/src/crate-map.md`; run the UI suite,
   the gallery's math pages in a nested release session, and compare a
   formula's pixels before/after for shapes whose widths legitimately differ.
6. Keep the replacement on a local branch until the user accepts the visual
   and metric differences; the repository's no-push rule applies.

## What the UI would gain

Beyond removing two dependencies, the replacement exposes a node tree with
byte spans and geometry. That is the substrate the math-editor spike said was
missing for tier ② (structural editing): `math-editor` already provides
caret paths, empty-slot rectangles and hit testing on the same layout. The
current UI edits a formula through a source line (`MathField`), so structural
editing remains a product decision, not part of the pipeline swap.

## How each claim was checked

- File and line references were read from the working tree on 2026-11-07.
- The contract list comes from `ui-core/src/math.rs` and `cx.rs`; callers come
  from a workspace-wide search for `ui_math`/`math_layout`.
- The diagnostic shapes were reproduced with mathstack's parser
  (`\frac{1}{1+`, `\frac{1}{1+\frc{1}{n}}`); `ui-math`'s expected behaviour is
  its own test `an_unclosed_group_still_lays_out_the_structure_above_it`.
- No UI code was modified; no release or graphical run was needed for this
  investigation.
