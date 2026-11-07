# M5, part 2: the long tail

The conformance checklist deferred twelve constructs. Five map onto nodes and
layout that already existed, so they were added by construction in the parser
and writer alone:

| Construct | Node |
| --- | --- |
| `\stackrel{a}{=}` | `OverUnder { base, over: a, under: None }` |
| `\substack{…}` | a `Table` named `substack`, which its script position already sets small |
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
`Table`. Its cells are script style and `Document::to_latex` records the name
`substack`, so the writer prints the command again instead of the structurally
equivalent `\begin{smallmatrix}`; a bare `to_latex` without spellings still
infers `smallmatrix`. Making a cell also close at `}` is what lets one
row-parsing routine serve a brace-delimited grid and an environment.

`\pmod` follows amsmath's expansion rather than guessing: a quad, then
`\text{(mod}`, a thin space, the argument, and `\text{)}`. The spaces are
`Space` nodes, so they do not participate in atom glue and the writer can print
each back exactly.

`\cfrac` sets its parts in display style, which is what the `Display` style on a
fraction means for `\dfrac`; the writer therefore prints it as `\dfrac` and the
round trip preserves the structure.

## Environments

`aligned` (and `align`, `align*`, treated the same inside a formula) is a
`Table` whose columns alternate right, left, right, …. Layout already places
cells by `Alignment`, so the environment only sets the per-column list.
The writer recognises that alternating list and prints `\begin{aligned}`, which
is how the alignment survives a round trip. `split` has the same table shape
and is told apart only by its recorded name.

`gather`, `gather*` and `gathered` are one centered column, which is a `Table`
with no explicit alignment; layout centers a column by default. `equation` and
`equation*` hold one formula, so the parser returns its row without a table.

`cases`, `dcases`, `rcases` and `drcases` are a table whose columns are all
left-aligned, wrapped in the brace its name selects: `cases` and `dcases` use a
left brace, `rcases` a right one, `drcases` both. The table shape cannot say
which name was written, so the parser records the environment name alongside
the spellings and `Document::to_latex` writes it back.

The mathtools `matrix*` family adds an optional `[l|c|r]` argument that applies
to every column; the parser stores it in the table's per-column alignment, and
the writer prints `\begin{matrix*}[l]` (or the fenced variant) when every column
shares an alignment. A table with no alignment stays `matrix`, and alternating
columns stay `aligned`.

`Table` carries an optional cell style, and layout sets each cell in
`cell_style.unwrap_or(style)`, so an environment can fix its cells independently
of where it appears. Matrix and cases cells use text style, `aligned`/`gather`/
`split` and the `d` cases use display style, and `smallmatrix` uses script
style. The writer infers `smallmatrix` from script cells and picks
`cases`/`dcases`/`rcases`/`drcases` from the fence and the cell style, while a
recorded environment name still wins over the inferred one.

`array` and `subarray` take a mandatory `{l|c|r}` column specification, so the
table gained two fields: `column_rules` counts the vertical rules at each
column boundary (one entry per boundary, empty when there are none) and
`column_spacing` says whether adjacent columns keep the half-em gap or sit
flush. The parser reads the preamble as text and interprets it: `l`, `c` and
`r` append a column, `|` adds a rule to the boundary reached so far, and any
other specifier is one diagnostic. `array` inherits the surrounding style,
which is what `cell_style: None` now means for a parsed table; `subarray` sets
its cells in script style. Layout widens a boundary that carries rules by their
thickness (0.04 em, LaTeX's `\arrayrulewidth`) and centres the rules in the
widened gap, so the columns keep their distance. A boundary with no rule and no
inner side adds no space, which keeps a matrix's width unchanged. The rules span
the table's ascent and descent and are emitted as `Rule` items.

`alignat`, `alignat*` and `alignedat` take a mandatory pair count `{n}`. The
parser declares `2n` columns aligned right/left in pairs and sets their spacing
flush; the source's own `\,` or `\quad` is then the only separation, unlike
`aligned`, whose columns keep a gap. That flush flag is also how the writer
tells the two apart, since their alignments are identical.

The writer rebuilds an environment's argument from the model rather than
keeping the source text, so `array{|c|c|}` and `alignat{2}` survive a round
trip. Canonical output with no recorded name spells `array` for a
style-inheriting table, `subarray` for a script table that carries an explicit
alignment (`smallmatrix` has none to carry) and `alignedat` for a flush display
table whose columns alternate.

## Style switches and atom classes

`\displaystyle`, `\textstyle`, `\scriptstyle` and `\scriptscriptstyle` scope
over the rest of the group, as TeX's style nodes do. The parser keeps a pending
switch while it reads a row and, at the row's end, wraps the siblings that
followed in `Styled { size, body }`; a second switch closes the first, so
`\displaystyle a\textstyle b` is two wrappers. A row is a group, so the wrapper
closes before an infix fraction's denominator and stops at a closing brace. A
style command in a position with no siblings (a one-atom argument) keeps an
empty body. Layout lays the body with the named level and clears the cramped
bit, as the named styles are uncramped; the wrapper gets a box of its own with
the body as its only child, so a caret path can enter it. The writer prints the
command and then the body without adding braces, which is the scope the source
had. A single-element body keeps that element's class for the spacing either
side; a longer body counts as Ord at the boundary, which is what KaTeX's
styling group does.

`\mathop`, `\mathbin`, `\mathrel`, `\mathord`, `\mathopen`, `\mathclose`,
`\mathpunct` and `\mathinner` wrap one math argument in `Class { class, body,
limits }`. Row spacing and `\mathop{…}\limits` see the wrapper's class rather
than its body's node types: `\mathop` carries the display-limits default,
`\limits`/`\nolimits` override it through the existing suffix path, and
layout's `takes_limits` reads the wrapper. The other classes never take limits,
so the suffix is a diagnostic after them. The writer prints the command, the
argument and, for an `Op`, the non-default suffix, so the canonical form keeps
an explicit class.

## Style-dependent bodies and struts

`\mathchoice{d}{t}{s}{ss}` keeps all four branches as the four children of
`MathChoice { display, text, script, script_script }`, each in
`Slot::Choice(Size)`, so a caret path, a copy and a structural edit still name
the branch by the style it belongs to. Layout lays out only the branch the
current style selects; the other three have no box, so they emit no glyphs, no
diagnostics and no geometry, which is what TeX's expansion does. Row spacing
asks `node_class` for the selected branch, so a `\sum` inside `\mathchoice`
spaces as an operator in display style. The branches are required slots:
deleting their contents leaves an empty row rather than removing a branch, and
the writer prints all four, so canonical `\mathchoice` round-trips exactly.

`\mathstrut` is `\vphantom{(}`. The parser builds a vertical `Phantom` whose
body is a `(` atom carrying the command's span, so the atom has no source of its
own to point at. Layout already gives a vertical phantom zero width and the
body's ascent and descent, and the em follows the current style, so the strut
is exactly a parenthesis tall at every level. No new node was needed. The
command's spelling is recorded, so a document writes `\mathstrut` back;
canonical output without spellings prints `\vphantom{(}`. The caret cannot
enter the body, as with any `\phantom`.

## Color

`Color { color, body }` wraps a body. `\textcolor` reads its color argument and
body directly; `\color` is a declaration, so it shares the scope rule of a
style switch. The pending declarations form a stack that closes latest-first:
a later declaration nests inside an earlier one, which keeps both scopes alive
to the end of the group. `\color{red}a\textstyle b` therefore stays red while
the later style governs `b` alone, and a later `\color` nests inside an
earlier one. Closing order also preserves source order, so
`\displaystyle\color{red}a` and `\color{red}\displaystyle a` keep their
different nestings; both render the same because an inner style overrides an
outer one. The preceding version closed a style switch before starting the next
(they were siblings); that cannot represent a color that spans the boundary, so
the switch now nests and the scope test pins that shape.

`Color` is straight sRGB with an alpha byte. The parser accepts 27 common
CSS/dvips names (case-insensitively, including the `aqua`/`fuchsia` and grey
aliases), bare six-digit hex as xcolor's HTML model, and `#RGB`, `#RGBA`,
`#RRGGBB`, `#RRGGBBAA`; the alpha forms extend KaTeX, which accepts only
three- and six-digit hex, while bare hex and case-insensitive names match it.
An unknown name or a malformed hex value is a diagnostic and the wrapper keeps
black, so the tree stays editable; the diagnostic is what surfaces the error.
Canonical output
writes a table name when one matches the value, else lowercase hex, with alpha
only when it is not opaque. KaTeX passes any alphabetic CSS keyword through to
the renderer, so names outside this table (for example `rebeccapurple`) are a
deliberate divergence: storing RGB keeps layout independent of CSS. Extending
the table is additive.

Display items carry `Option<Color>`. Layout does not thread a color parameter:
`layout_color` lays its body out, then walks the finished box tree and fills
every item that is still uncolored. An inner `\textcolor` has already claimed
its items, so it wins over an outer declaration; rules and lines are items of
the same boxes, which is why a fraction bar or radical is colored by the same
walk. `math-svg` writes `fill`/`stroke` per item and leaves the group's black
default for uncolored items, so the demo, which paints through `math-svg`,
needs no separate color path.

`Document::to_latex` keeps a `\color` declaration as `\color{name}` through a
spelling that matches the node's color. A declaration's body is the rest of its
group, so the writer leaves it unbraced only while it is the last item of its
row; when a sibling follows — a fragment inserted or pasted mid-row — the body
is wrapped in braces so the scope ends there. A style switch follows the same
rule, and a script base is written as if a sibling followed, keeping the script
out of a declaration's body. `\textcolor` has no spelling because
its canonical form already is the braced command. Canonical output writes every
color as `\textcolor{…}{…}`; a declaration over a single item keeps that item
as its body, so the braced form re-parses to the same tree. The wrapper counts
as a group for atom spacing (one element keeps its class, more count as Ord),
the same approximation the style switch makes; KaTeX's fragment behavior for a
colored multi-atom group is not reproduced.

`\colorbox{color}{text}` and `\fcolorbox{frame}{background}{text}` are
`ColorBox { background, border, body }`; their body is a text-mode argument,
like `\text`, not a math list. `\colorbox` pads by `\fboxsep` (0.3 em) and
draws no frame; `\fcolorbox` adds the `\fboxrule` (0.04 em) frame on top of
that padding, fills with the background color and draws the frame in the border
color. The writer derives the command from the border and prints the two color
arguments in order, so both forms round-trip. Only a `Text` body is written: an
`Error` from a missing argument is omitted, so re-export keeps the error, and a
math body built through the public API is refused rather than flattened into
literal text. An unknown color is the same diagnostic and black fallback as
`\textcolor`. This follows TeX's `\fbox`
model, which puts the rule outside the separation on every side. KaTeX's
border-box frame spans the padding horizontally instead, so its `\fcolorbox`
is 0.08 em narrower than this stack; the vertical extent agrees. KaTeX also
admits `\colorbox` inside `\text`; commands inside this stack's `\text` stay
literal, so the box is math-mode-only here.

A background is not a rule drawn first. `Item::Background` is its own primitive:
`MathBox::flatten` emits a box's backgrounds before its children and its other
items, and `math-svg` paints every background before any glyph, rule or line,
so the result does not depend on where the item sits in the list. The fill
covers the padded box, including the frame area; the frame rules are painted
over it in their own color, and an outer `\color` does not repaint either.

## Top-level line breaks

`\\` and `\newline` outside a table are not modelled, and `\par` is a
diagnostic. The decision rests on the sources rather than on rendering alone:

- KaTeX 0.16.9 registers `\\` as `cr`, whose builders run only at the top level
  (`src/functions/cr.js`); `\newline` is the macro `\\\relax`
  (`src/macros.js`). Outside arrays it produces a hard break in inline mode,
  and in display mode it breaks unless `strict: "error"` applies the
  `newLineInDisplayMode` rule (`src/Settings.js`, `docs/options.md`). A
  `\\[len]` becomes a margin above the next line.
- LaTeX's `\[...\]` and `equation` are single-line display environments; line
  breaks belong to `align`/`gather`/`multline`. The exact first error for
  `\[a\\b\]` was not reproduced locally because the installation lacks format
  files; the reference documentation is consistent that it is an error
  `[unverified]`.
- A single-column `Table` is not an equivalent model: it exposes cells and
  alignment, fixes a 0.5 em row gap, and canonical output would name a
  `gathered`/`array` environment instead of the source form. It would also
  change the editor's root and caret assumptions.

Revisit only when a host document needs multi-line formulas. The correct
increment then is a first-class line-break leaf with multi-line layout and
selection geometry, not a table wrapper. Until then both spellings stay an
unknown-command diagnostic, which is faithful to LaTeX and visible.

## Decorated operators and `\displaylimits`

`\varlimsup`, `\varliminf`, `\varinjlim` and `\varprojlim` are amsmath's
decorated `lim`; KaTeX expands them to `\operatorname*{\overline{lim}}`,
`\operatorname*{\underline{lim}}`, `\operatorname*{\underrightarrow{lim}}` and
`\operatorname*{\underleftarrow{lim}}`. The parser builds that same tree: a
`Function { name: "lim", limits }` under the decoration, wrapped in
`Class { class: Op, … }`, so limit placement and atom spacing still see an
operator. The model gained `UnderArrow { symbol, body }` for the two arrow
forms.

The arrow is not `Arrow`. An `Arrow` is a relation whose labels sit above and
below a stretched arrow, and it is at least one em wide; a varlim's arrow hangs
below a full-size `lim`, adds only descent, and the wrapper supplies the
operator class. Reusing `Arrow` would have made `\varinjlim` a relation and
lifted `lim` off the baseline, so `UnderArrow` stretches the arrow across the
body, clears it by the bar gap, and centres it under a body laid out at full
size.

`Document::to_latex` prints the written command back through the
`DecoratedOperator` spelling, which re-checks the wrapper class, the limits and
the whole decorated body before using the name, so an edit that moves one of
them falls back to canonical output instead of keeping a stale name. Canonical
output writes the expanded form, `\mathop{\overline{\operatorname{lim}}}_{x}`,
which re-parses to the same tree.

`\displaylimits` is the postfix that selects display placement; it is not
"restore the operator's table default". On a large operator, a named function
or a `Class` of class `Op` it sets `Default { display_limits: true }`, so
`\operatorname{lim}\displaylimits` stacks its scripts in display style while
`\operatorname*{lim}` and `\sum` are unchanged. After any other class the
suffix is a diagnostic. The writer omits it where the operator's own default is
already display limits (`\sum\displaylimits` prints `\sum`) and keeps it where
the default is side placement (`\int\displaylimits`), since only the latter
would change meaning. `\limits` and `\nolimits` still override either way.

A named function's recorded spelling is reused only when it reproduces the
recorded placement without a suffix; `\operatorname{lim}\displaylimits`
therefore writes `\lim`, while `\operatorname*{lim}` stays itself.

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

`\binom`, `\dbinom` and `\tbinom` are amsmath's `\genfrac` shorthands: a
`\left(…\right)` pair around a zero-thickness fraction, in the inherited,
display or text style. They parse into that same `Delimited` + ruleless
`Fraction` structure rather than a new node, and the writer recognises the shape
and prints the binomial command back, including when the source spelled it as
`\left(\genfrac{}{}{0pt}{}{…}{…}\right)`.

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
  label, both braces, `\cancel`/`\bcancel`/`\xcancel`, three `\hspace` lengths,
  `array` with a mixed preamble, `array{|c|c|}`, a rules-free `array`,
  `subarray`, and both `alignat` spellings; the four varlim commands, both
  under-arrows, and `\displaylimits` after `\operatorname`, `\operatorname*`,
  `\sum` and `\int`.
- `table_boundaries_carry_rules_and_flush_spacing` pins the three rule
  positions, thickness and full-height span of a `|c|c|`-shaped table, and that
  flush columns sit at zero gap; ignoring the rule width or the spacing flag
  fails it.
- `array_and_alignat_arguments_shape_the_table` pins the preamble mapping
  (`l|c|r` to alignments and boundary counts), that `array` inherits the cell
  style, `alignat`'s pair count and flush spacing, and that the writer keeps
  the name and the column specification; hard-coding a cell style or swapping
  the spacing fails it.
- `matrix_environments_build_rows_and_cells` checks that `aligned` yields
  `[Right, Left]`; flipping either entry makes it fail.
- `style_switch_scopes_to_the_end_of_its_group` pins that a switch covers the
  rest of its group only and that a second switch closes the first; wrapping
  the whole row or ignoring the scope start fails it.
- `class_wrappers_keep_their_class_and_limits` pins each command's class and
  `\mathop`'s display-limits default, and that `\limits` survives the writer.
- `mathop_takes_limits_in_display_style` lays `\mathop{f}_x` at both styles:
  the subscript is centred below in display style and beside in text style;
  dropping the wrapper arm in `takes_limits` fails it.
- `varlim_commands_are_display_limit_operators` pins each command's `Op` class,
  display limits, decoration and `lim` body; dropping the class or the limits
  fails it.
- `displaylimits_selects_the_display_placement` pins that the suffix turns a
  side-placed `\operatorname` into display limits while `\sum` and
  `\operatorname*` stay unchanged.
- `display_limits_stack_a_function_under_it` lays a display-limited function
  and the varlim shape at both styles: the subscript stacks in display style;
  removing the `Function` arm in `takes_limits` fails it.
- `mathchoice_lays_out_only_the_branch_for_the_style` pins a different width
  at each of the four styles; always choosing one branch fails it.
- `mathstrut_shape_keeps_the_paren_extent_in_every_style` pins zero width and
  the paren's ascent and descent at text and script sizes; laying the phantom
  out as full or horizontal fails it.
- `mathchoice_keeps_four_branches_in_style_order` pins each branch's symbol and
  the written form; swapping two branches in the parser fails it.
- `mathstrut_is_a_vertical_paren_phantom` pins the node shape, the retained
  spelling and the canonical `\vphantom{(}`; dropping either fails it.
- `color_reaches_nested_glyphs_and_rules` pins that an inner color keeps its
  glyph while an outer one reaches a fraction's bar; overwriting every item in
  the tint walk fails it.
- `color_declarations_scope_to_the_group_and_nest` pins the declaration body
  covering the following siblings and a `\textcolor` nested inside it;
  dropping the declaration spelling fails it.
- `color_and_style_switches_keep_their_source_order` pins both orderings of
  `\color` and `\displaystyle`; closing declarations oldest-first fails it.
- `textcolor_and_color_write_back_their_own_forms` pins the two written forms,
  the canonical re-parse, hex output, case-insensitive names, bare six-digit
  hex, an alias, and the unknown-name diagnostic.
- The `math-svg` `constructs` example rendered a red/blue/green formula whose
  SVG parses as XML; glyphs carry the expected `fill` and both rules inside the
  blue scope carry it too.
- `colorbox_background_sits_behind_the_text_and_the_border_keeps_its_color`
  pins the `\fboxsep` + `\fboxrule` geometry, that the background is the first
  item, and that all four frame rules use the border color; dropping the
  background-first flatten order or sharing the background color with the frame
  fails it.
- `colorbox_arguments_are_text_and_write_back_their_commands` pins the
  text-mode body, both written forms, the canonical re-parse and the
  unknown-color diagnostic; parsing the body as math or swapping the frame and
  background arguments fails it.
- `a_colorbox_without_a_body_writes_an_error_back` pins that a missing body is
  omitted from the output and re-parses as the same error; writing the Error
  node as text heals the diagnostic and fails it.
- A manual `math-svg` render of `\colorbox{yellow}{Hi}+\fcolorbox{blue}{red}{X}`
  parses as XML with the yellow and red fills before the text paths and four
  blue frame rules after them.
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

## Corrections

- **A style-inheriting table was written as a matrix.** Canonical output spelled
  every table without a cell style `\begin{matrix}`, but a matrix sets its cells
  in text style, so reading the output back changed the table's style. The
  writer now spells `array` with a preamble rebuilt from the per-column
  alignment, since `array` is the environment that inherits. The round-trip
  corpus entry `\begin{array}{cc}…` fails if the inference goes back.
- **The table count was all the model could say about columns.** `array` was
  rejected because its specification had nowhere to live, and a matrix has no
  place for a vertical rule. `column_rules` stores one count per boundary
  rather than one flag, because `||` draws two rules with no space between
  them; layout emits one `Rule` item per count. Dropping the bars from the
  writer's preamble fails the round-trip corpus, and ignoring the counts in
  layout fails the rule-geometry test.
- **`aligned` and `alignedat` had the same shape.** Both parse to alternating
  right/left display columns, so the writer could not tell them apart; the
  canonical form wrote `aligned` for both. The distinct spacing (`Spaced` and
  `Flush`) is now part of the table and selects `alignedat`, which also carries
  its pair count. `array_and_alignat_arguments_shape_the_table` asserts the
  canonical name, so dropping the flush flag makes it fail.
- **Per-atom styling would have hidden the switch.** Rewriting the kinds or
  styles of the atoms after `\displaystyle` would have changed their spacing
  and drawing but left no node for the switch: no box to hit, no caret stop,
  and nothing the writer could print back. The switch is a wrapper with a body
  slot, which is what KaTeX's `styling` node does; the scope test fails if the
  wrapper boundary moves.
- **A background cannot reuse `Rule` with an earlier position.**
  `MathBox::flatten` emits children before a box's own items, so a rule used as
  a fill would cover the content it should sit behind; moving it earlier would
  make the result depend on call order. `Item::Background` is a separate
  primitive with its own paint pass.
  `colorbox_background_sits_behind_the_text_and_the_border_keeps_its_color`
  fails if the background stops preceding the content or the frame takes the
  background color.
- **A declaration written unbraced assumed it was last in its row.** Inserting
  or pasting `\color{red}y` between two atoms produced `a\color{red}yb`, whose
  reparse colors `b` too. The writer now wraps a declaration body in braces
  whenever a sibling follows, and writes a script base as if one did; the style
  switch uses the same rule. `a_declaration_inserted_mid_row_keeps_its_scope`
  fails without the braces (previously it exported `a\color{red}yb`).
- **A missing colorbox body was written as the command's own name.** The
  writer's text-argument fallback serialized the `Error` node inside the text
  argument, so `\colorbox{red}` exported as `\colorbox {red}{\colorbox }` and
  re-parsed cleanly as literal text. Only a `Text` body is written now;
  `a_colorbox_without_a_body_writes_an_error_back` fails if the error heals.
- **A style switch could not stay a sibling of the next one.** Colors exposed
  the old rule: closing the previous switch when a new one starts loses scope
  for a `\color` that spans the boundary. Declarations are now a stack closed
  latest-first, so each keeps its scope to the group end and a later one nests
  inside. `style_switch_scopes_to_the_end_of_its_group` and
  `color_and_style_switches_keep_their_source_order` pin the nesting and the
  source order.
- **Following KaTeX's `\mathop` flag would put its limits at the side.** KaTeX
  0.18.9 marks `\mathop` with `limits: false`, so its scripts stay beside even
  in display style. TeX makes every Op atom `\displaylimits` by default, and
  `\mathop{…}_x` in display style is expected above and below; the wrapper
  therefore starts with `Default { display_limits: true }`, and
  `mathop_takes_limits_in_display_style` pins the placement.
- **`\underrightarrow` cannot reuse `Arrow`.** `Arrow` is a relation whose
  labels surround a stretched arrow and whose arrow is at least one em; a varlim
  is an ordinary full-size `lim` with a stretchy arrow below it. Mapping the
  command onto `Arrow` would have made the operator a relation and moved `lim`
  off the baseline, which no later writer pass can undo. `UnderArrow` states
  the vertical order directly, and
  `varlim_commands_are_display_limit_operators` fails if the decoration is
  anything else.
- **`\displaylimits` is not "reset to the operator's table default".** It
  selects display placement, so on `\int` it must be written back: canonical
  output that dropped every `Default { true }` suffix read back as side scripts.
  The writer compares against the operator's intrinsic default —
  `\sum\displaylimits` prints `\sum`, `\int\displaylimits` keeps the
  suffix — and the round-trip corpus contains both, so losing either half
  fails.
- **A spelling can outlive the placement it named.** Keeping the recorded
  `\operatorname{lim}` text for `\operatorname{lim}\displaylimits` printed a
  source that parses back with side scripts. The writer now re-derives the
  spelling's placement (`spelled_function_limits`) and falls back to `\lim`
  when it differs;
  `displaylimits_selects_the_display_placement` and the corpus catch it.
