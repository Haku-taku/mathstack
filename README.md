# mathstack

A standalone LaTeX math stack in Rust: one model for display and editing,
geometry computed by the engine, and a flat display list a host paints.

Status: **M5 complete; M6 started**. `math-core` is an id-stable arena: it classifies
atoms, applies the TeX spacing tables and lays out rows, scripts, fractions,
radicals, `\left…\right` delimiters, large operators with limits, accents,
text, bars, fixed and middle delimiters, named operators, the phantom family
and matrix-style tables. Over the same boxes it builds caret paths, hit testing
and selection, so an empty slot still returns a caret rectangle. `math-syntax`
lexes LaTeX, expands token-list macros, parses the supported grammar with error
recovery into the arena, including the matrix, `cases` and `aligned`
environments, and writes it back; the curated checklist parses all 77
constructs cleanly (100%).
`math-font` reads advances, ink, MATH constants,
stretch variants, `ssty` alternates and accent attachments through
`ttf-parser`, including assembly of tall glyphs; text is measured through a
host `TextShaper` with a same-face fallback, and `math-svg` paints the display
list as outline geometry. The 16-formula differential against formulary parses
its corpus from LaTeX and is recorded in
[`docs/m1-differential.md`](docs/m1-differential.md); the language milestones
and the editing model are in [`docs/m2-parser.md`](docs/m2-parser.md),
[`docs/m3-macros-text.md`](docs/m3-macros-text.md) and
[`docs/m4-editing.md`](docs/m4-editing.md) and
[`docs/m5-tables.md`](docs/m5-tables.md) and
[`docs/m5-long-tail.md`](docs/m5-long-tail.md) and
[`docs/m5-editing.md`](docs/m5-editing.md). The per-edit budget is measured in
[`docs/m5-budget.md`](docs/m5-budget.md), and the completeness gates are in
[`docs/completeness.md`](docs/completeness.md). `math-editor` is the host
adapter: structural typing, caret, selection, clipboard, commands, IME and
undo over the model, with a native eframe demo in `math-editor-demo`; see
[`docs/host-editor.md`](docs/host-editor.md).

## Crates

| Crate | Responsibility |
| --- | --- |
| `math-core` | Arena model with spans and ids, atom classes, spacing, style levels, layout, editing geometry, display list, font traits |
| `math-syntax` | LaTeX lexer, macro expander, recovering parser, LaTeX writer |
| `math-font` | `MathFont` and `GlyphOutline` over one SFNT face; metrics converted to em |
| `math-svg` | Display list to SVG paths and rules |
| `math-editor` | Host editing layer: caret, selection, keystrokes, commands, IME and undo |
| `math-editor-demo` | Native eframe host: input, `resvg` painting, source field and toolbar |
| `math-diff` | Differential extents and items against formulary (development tool) |

## Build and run

```sh
cargo test --workspace
```

```sh
cargo run -q -p math-svg --example render -- \
  /usr/share/fonts/wps-office/DejaVuMathTeXGyre.ttf "a+b=c" > row.svg
```

```sh
resvg row.svg row.png
```

```sh
cargo run -q -p math-svg --example constructs -- \
  /usr/share/fonts/wps-office/DejaVuMathTeXGyre.ttf \
  'x^2+\frac{a}{b}=\sqrt[3]{c}' > constructs.svg
```

The editor demo opens a window on the same font:

```sh
cargo run --release -p math-editor-demo -- \
  /usr/share/fonts/wps-office/DejaVuMathTeXGyre.ttf \
  '\frac{-b\pm\sqrt{b^2-4ac}}{2a}'
```

The differential report parses every corpus formula from LaTeX, pairs it with
its MathML and compares extents against formulary:

```sh
cargo run -p math-diff -- /usr/share/fonts/wps-office/DejaVuMathTeXGyre.ttf --markdown
```

The demo takes any OpenType face with a MATH table; the path above is the
DejaVu Math TeX Gyre copy installed on this machine, not bundled.

## Design

`docs/architecture.md` is the target design and its corrections,
`docs/survey.md` the source-grounded comparison of ten stacks, and `docs/raw/`
the per-library notes and the TeX/MATH algorithm references. Reuse of upstream
code and data is recorded in [`THIRD-PARTY.md`](THIRD-PARTY.md).
