# M5, part 2: the long tail

The conformance checklist deferred twelve constructs. Five map onto nodes and
layout that already existed, so they were added by construction in the parser
and writer alone:

| Construct | Node |
| --- | --- |
| `\stackrel{a}{=}` | `OverUnder { base, over: a, under: None }` |
| `\substack{…}` | a `Table`, which its script position already sets small |
| `\cfrac{…}{…}` | `Fraction` with style `Display`, like `\dfrac` |
| `\pmod{n}` | a row: quad, `\text{(mod}`, thin space, `n`, `\text{)}` |
| `\bmod` | a row: medium space, `\text{mod}`, medium space |

The other seven needed a layout capability or a model field, and are described
below.

## Long tail on existing nodes

`\stackrel` reuses the `\overset` arm; only the command name differs, and the
writer normalizes both to `\overset`.

`\substack` parses its braces as a grid: rows end at `\\`, cells at `&`, and
the closing brace ends the grid. The environment parser and `\substack` share
`build_table`, which pads every row to the widest and wraps the rows in a
`Table`. The writer prints it as `\begin{matrix}`, which parses back to the same
structure. Making a cell also close at `}` is what lets one row-parsing routine
serve a brace-delimited grid and an environment.

`\pmod` follows amsmath's expansion rather than guessing: a quad, then
`\text{(mod}`, a thin space, the argument, and `\text{)}`. The spaces are
`Space` nodes, so they do not participate in atom glue and the writer can print
each back exactly.

`\cfrac` sets its parts in display style, which is what the `Display` style on a
fraction means for `\dfrac`; the writer therefore prints it as `\dfrac` and the
round trip preserves the structure.

## Aligned environments

`aligned` (and `align`, `align*`, treated the same inside a formula) is a
`Table` whose columns alternate right, left, right, …. Layout already places
cells by `Alignment`, so the environment only sets the per-column list.
The writer recognises that alternating list and prints `\begin{aligned}`, which
is how the alignment survives a round trip.

## Lengths and `\hspace`

`\hspace` needs a width in a unit, so the model gained:

```rust
pub enum LengthUnit { Em, Ex, Mu }
pub struct Length { pub milli: i32, pub unit: LengthUnit }
```

Integer thousandths keep nodes comparable, and the writer can reproduce the
value exactly. Layout resolves the unit where it has the font: `em` scales by
the current em, `ex` by the font's x-height, and `mu` by one eighteenth of an
em. Physical units are not represented, because the model has no resolution to
convert them with; the parser reports one as an unsupported length. `Hspace`
takes width but, like `Space`, no part in atom classes or glue.

`\genfrac`'s thickness uses the same type. Its common `0pt` spelling is
accepted because zero is the same length in every unit; a nonzero physical unit
is still a diagnostic.

## `\genfrac`

A fraction's rule and style were implicit (`bar: bool`, `forced:
Option<bool>`). `\genfrac` exposes both, so the fields became:

```rust
Fraction { numerator, denominator, rule: FractionRule, style: Option<Size> }
enum FractionRule { None, Default, Custom(Length) }
```

`\frac`, `\dfrac`, `\tfrac` and `\cfrac` set `Default` plus an optional style;
`\atop` sets `None`. Layout draws a rule unless the rule is `None`, using the
custom length or the MATH constant, and the forced style replaces the
surrounding level before the fraction's own numerator/denominator styles apply.
The writer picks the named command when it can spell the combination and
`\genfrac` otherwise (a custom thickness or a script-level style).

`\genfrac`'s delimiter arguments wrap the fraction in `Delimited`, and a part
that the writer will print through `\atop` is wrapped in a row to match what
the infix parse produces; without that the structure round trip differed only
in the wrapper.

## `\cancel` and line items

The display list had glyphs and axis-aligned rules only. A cancellation is a
diagonal, so it gained `Item::Line(LineItem)`; `math-svg` paints it as a stroked
`<line>`. `Cancel { body, up, down }` draws the body then one or both diagonals
across its box, with a 0.04em stroke. `\cancel`, `\bcancel` and `\xcancel` are
the three flag combinations.

## Horizontal stretch

The font trait already grew glyphs vertically; it gained
`stretch_horizontal`, implemented over the MATH table's horizontal
constructions with the same three-step protocol and the same assembly code (the
part offsets count from the start instead of the bottom). Layout emits an
assembly's parts along x.

- `Arrow { symbol, over, under }` is `\xrightarrow`/`\xleftarrow`: the arrow is
  stretched to the wider label plus 0.5em (at least 1em), the labels sit above
  and below it, and the node is a relation.
- `HorizontalBrace { body, over }` is `\overbrace`/`\underbrace`: the brace
  glyph (`⏞`/`⏟`) is stretched across the body and clears it by the bar gap.
  Its scripts stay above/below in every style, unlike a large operator's
  movable limits.

Both fall back to the plain glyph and a diagnostic when the face has no arrow
or brace glyph.

## Verification

- Round-trip sources gained `\stackrel`, `\substack` with two rows, `\cfrac`,
  both modular commands, an `aligned` block, four `\genfrac` spellings (zero,
  custom, script style, delimiter pair), both arrows with and without an under
  label, both braces, `\cancel`/`\bcancel`/`\xcancel`, and three `\hspace`
  lengths.
- `matrix_environments_build_rows_and_cells` checks that `aligned` yields
  `[Right, Left]`; flipping either entry makes it fail.
- `hspace_resolves_em_ex_and_mu_units` pins the three unit conversions and a
  negative length; changing the `mu` scale makes it fail.
- `cancel_draws_one_diagonal_per_direction` pins the single-line and both-line
  forms and the direction of each diagonal; swapping the endpoints fails it.
- `genfrac_uses_its_rule_thickness_and_style` pins a 10pt custom rule, no rule,
  and a script-level style; falling back to the default thickness fails it.
- `xrightarrow_stretches_to_its_label` pins the assembly's two parts at the ends
  of the stretched arrow; disabling the horizontal stretch fails it.
- `braces_sit_on_the_body_and_keep_their_scripts_outside` pins the vertical
  order body → brace → superscript and that the script stays over the brace;
  removing the forced limits fails it.
- The checklist moved all twelve constructs to `CORE`: 77/77 = 100%.
  `tracked_gaps_are_still_gaps` failed with exactly the expected names at each
  step, which is what forced each promotion; with the list empty that test is
  retired. Parse coverage against KaTeX's own list is measured separately in
  [`completeness.md`](completeness.md).

## Remaining in M5

Closed. The per-edit budget is in [`m5-budget.md`](m5-budget.md); structural
edits, `Origin::Macro` and the `Composition` node are in
[`m5-editing.md`](m5-editing.md).
