# M2: parser, spans and writer

`math-syntax` turns LaTeX math source into the `math-core` model tree, keeping a
byte span for every node, and prints a tree back to LaTeX. This page records the
grammar, the recovery rules and what the round-trip test actually proves.

## Grammar

```
formula    := row
row        := item*
item       := atom ( '^' argument | '_' argument )*
atom       := symbol | group | command
group      := '{' row '}'
argument   := group | atom
command    := '\frac' argument argument
            | '\sqrt' ( '[' row ']' )? argument
            | '\left' delimiter row '\right' delimiter
            | large-op
            | symbol-command
```

- Whitespace is skipped; `%` starts a comment that runs to the newline.
- `\left`/`\right` bodies are rows terminated by `\right`, not brace groups, so
  the body may contain `^`, `_` and nested `\left…\right`.
- A delimiter is one character, a delimiter command (`\{`, `\langle`, `\|`), or
  `.` for TeX's null delimiter.
- `\sqrt[3]{x}` parses the index as a bracket-terminated row.
- The accepted symbol commands and their Unicode codepoints are in
  `crates/math-syntax/src/symbols.rs`; they follow unicode-math, as KaTeX's
  symbol tables do.
- Direct Unicode input (`α`, `∑`, `≤`) is a symbol atom too; `∑` typed directly
  is an `Atom` of class Op, not a `LargeOp`, because no `\sum` command was seen.
- `\limits`, `\nolimits` and `\displaylimits` are postfix: they follow an
  operator before any script. `\displaylimits` selects display placement; it
  does not restore the operator's table default. After another class it is a
  diagnostic.
- Named operators come from one table: `\arg` and `\ker` are side-placed,
  `\argmin` and `\injlim` follow `\operatorname*`, and `\plim` is above/below
  in every style because `\limits` is part of its definition. `\varliminf`,
  `\varlimsup`, `\varinjlim` and `\varprojlim` expand as amsmath does: a
  decorated `lim` (`Underline`, `Overline`, or `UnderArrow`) inside an `Op`
  class, so display limits still see an operator.

## Mapping to the model

| Source | Node |
| --- | --- |
| symbol or symbol command | `Atom { symbol, class }` |
| `{ … }` | `Row` |
| `base^a_b` | `Scripts { base, sup, sub }` |
| `\frac` | `Fraction { bar: true }` |
| `\sqrt` | `Radical` |
| `\left…\right` | `Delimited` |
| `\mathbb`, `\mathbf`, … | `Variant { variant, body }` |
| `\sum`, `\int`, … | `LargeOp { symbol, limits: Limits }` |
| `\sin`, `\lim`, `\argmax`, … | `Function { name, limits: Limits }` |
| `\operatorname*` / `\operatornamewithlimits` | `Function` with `Default { display_limits: true }` |
| `\limits` / `\nolimits` after an operator | `Limits::AboveBelow` / `Beside` |
| `\mod{x}` / `\pod{x}` | a spacing row, like `\bmod` / `\pmod` |
| `\binom`, `\dbinom`, `\tbinom` | `Delimited` around a ruleless `Fraction` |
| unknown command | `Error(raw)` plus a diagnostic |

Scripts attach in either order and leave `sup`/`sub` as `Option`. A second
superscript or subscript reports `double superscript` and keeps the first.

## Spans

A node's span covers the source of its construct, including delimiters: an atom
its token, a row from its first child to its last, a group from `{` to `}`, a
fraction from `\frac` through its last argument's closing brace, and `\left…\right`
through the right delimiter.

A braced argument with exactly one child returns that child rather than a row —
`x^{2}` and `x^2` must produce the same model — and the child's span widens to
include the braces. Two consequences:

- `{a}` in an item position stays a row, so `a{+}b` keeps the plus ordinary
  during spacing.
- A genuine source one-item row can still be written: the writer doubles the
  braces (`{{a}}`) so the parser's unwrapping produces a row. A synthetic row
  is not a group the user wrote, so it does not get the extra pair.

## Recovery

The parser never aborts; it records a
`Diagnostic { span, message, kind }` and keeps the surrounding structure. The
kind separates `Incomplete` input (an unclosed group or missing argument, which
a host may treat as still typing) from `UnknownCommand`, `Layout` and other
errors.

| Input | Result |
| --- | --- |
| `a+\unknown+b` | row of five items, `Error("\unknown")` in place, one diagnostic |
| `\frac{a}` | fraction with an empty row denominator, one diagnostic at the end |
| `\left( x` | delimited node with `right: None`, body parsed, one diagnostic |
| `{a` | row keeps `a`, span reaches the end, `unclosed {` diagnostic |
| `a+\right)` | `\right` diagnosed and dropped, row continues |
| `x^2^3` | first superscript kept, `double superscript` diagnostic |
| 65 nested groups | inner row skipped non-recursively, `formula nested too deeply` |

Two budgets bound the work: 64 levels of nesting and 100 000 nodes. They turn a
stack overflow or an unbounded allocation into a diagnostic.

## Writer

`to_latex` writes the grammar back, normalized:

- commands are followed by a space (`\frac {a}{b}`) because an alphabetic
  command ends only at a non-letter;
- a row in an item position gets braces, a row as an argument does not (the
  caller supplies them); a source one-item row as an argument gets a second
  pair, while a synthetic one (an editor's typing container) gets only the
  argument braces;
- atoms that are syntax characters are escaped (`\{`, `\%`, `\backslash`);
- a symbol with no preserved spelling is written as its Unicode character,
  which the lexer accepts as a symbol;
- a missing argument is written as `{}`, a parsed error as its raw text plus a
  separating space.

Semantics alone cannot distinguish `\pm` from `±`, `\le` from `\leq`, or a
font-command atom from its bare letter. A `Document` therefore carries a
`Spellings` side table: the parser records the token for atoms, text, large
operators, functions, spaces, accents, sized delimiters and `\middle`, keyed
by node id and tagged with the shape it described. `Document::to_latex` emits
that token while the node still matches; an edited, synthesized or
macro-expanded node has no spelling and falls back to the canonical form.
`to_latex(model, root)` remains the canonical export for callers without the
table. Tokens and node ids travel together through copy and paste; deleting a
subtree drops its spellings so a reused id cannot inherit one.

The test proves structure, not bytes: for each corpus source,
`parse(to_latex(parse(s)))` equals `parse(s)` with spans erased, and
`to_latex` is idempotent on the second tree. Redundant braces and whitespace
therefore normalize away, while a construct the grammar cannot express does not
appear in a parsed tree.

Bar-less fractions (`bar: false`) come from `\atop`; the writer prints such a
fraction as `\atop` between its parts, so the rule-less form round-trips.

## Not yet

Macros, accents, font commands, `\text`, spacing commands, `\middle`, the
`\big` family, `\atop` and `\operatorname` are covered in
[`m3-macros-text.md`](m3-macros-text.md). Tables and environments are in
[`m5-tables.md`](m5-tables.md) and [`m5-long-tail.md`](m5-long-tail.md); the
host `TextShaper` remains open. Unknown commands become `Error` nodes, so the
rest of a formula still parses and edits.

## Verification

- `cargo test -p math-syntax` — spans, command mapping, every recovery branch,
  writer round-trip and idempotence.
- `cargo run -p math-diff -- <font.ttf> --markdown` — the corpus is now LaTeX
  source parsed by `math-syntax`; the extents and deltas in
  `m1-differential.md` are unchanged, which is the check that the parser builds
  the same trees the hand-written corpus did.
