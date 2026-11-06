# How existing math stacks are built

A survey of ten LaTeX/MathML stacks, read at source level, to decide the shape of
an owned math stack. Detailed per-library notes are in `raw/` (`js.md`,
`rust.md`, `algorithm.md`); this page is the synthesis and the design input.

## What was read

| Stack | Version | License | Core size | Read from |
| --- | --- | --- | --- | --- |
| KaTeX | 0.16.22 | MIT | ~20k JS LOC | GitHub source |
| MathJax | 4.1.3 | Apache-2.0 | ~106k TS LOC | GitHub source |
| Temml | 0.14.0 | MIT | ~13k JS LOC | GitHub source |
| MathLive | 0.111.4 | MIT | ~65k TS LOC | GitHub source |
| MathQuill | 0.10.1 | MPL-2.0 | ~11k TS LOC | GitHub source |
| formulary | 0.3.0 | MIT OR Apache-2.0 | ~4.5k Rust LOC | cargo registry |
| pulldown-latex | 0.8.0 | MIT | ~7.0k Rust LOC | cargo registry |
| mathtex | 0.2.0 (+editor 0.4.1) | MIT OR Apache-2.0 + TeX notices | ~11k Rust LOC + 58k generated | cargo registry |
| ratex | 0.1.15 | MIT | ~28k Rust LOC | crates.io |
| Typst (math) | 0.15.1 | Apache-2.0 | ~10k Rust LOC | GitHub source |

Sizes exclude tests and docs. Every structural claim below has a file:line in
`raw/`; the algorithm section cites TeXbook sections and the OpenType MATH spec.

## The pipeline everyone converges on

```
source → tokens → macro expansion → parse tree → semantic IR → boxes → paint
```

No two stacks put the same work in the same stage:

- **KaTeX** keeps one parse tree and builds two backends (HTML, MathML) from it.
- **MathJax** parses into `MmlNode` (MathML is the IR), then wraps it in output
  wrappers that compute `BBox`.
- **formulary** starts at MathML, so its "parse tree" is an XML tree.
- **pulldown-latex** stops at an event stream; it has no tree at all.
- **mathtex** runs a real XeTeX engine and lowers its node list into a retained
  IR with dense ids.
- **ratex** is a KaTeX port: parse tree → `LayoutBox` tree → flat display list.
- **Typst** has syntax → content → a resolved math IR → retained frames.

The useful invariant: **the semantic layer must be a tree whose nodes survive
into layout**, or editing geometry has nothing to point at. Only mathtex, Typst,
ratex (partially) and MathLive satisfy it; formulary, pulldown-latex and KaTeX
do not.

## Where geometry comes from

Three families:

1. **Browser lays out** (Temml, MathQuill, most of MathJax/KaTeX's horizontal
   text). Cheap to build, impossible to make deterministic or incremental, and
   the geometry belongs to a font the engine did not choose.
2. **Engine computes vertical metrics, CSS lays out horizontally** (KaTeX
   vlists, MathJax CHTML, MathLive). This is the classic compromise: correct
   baselines, browser-dependent advances.
3. **Engine computes everything** (Typst, mathtex/XeTeX, formulary, ratex).
   Deterministic, testable, needs real font metrics and a box model.

A native stack has to be family 3. That immediately implies: own box tree, own
vlist equivalent, metrics from font tables, and a display list as the only
output.

## Spans: where they die

| Stack | Spans survive to |
| --- | --- |
| KaTeX | parse nodes and diagnostics; builders ignore `loc` |
| MathJax | stack-item source slices for `data-latex-item`; not general |
| Temml | same as KaTeX |
| MathLive | none (tokens are strings); errors have no position |
| MathQuill | none |
| pulldown-latex | none in events; byte pointers only for error excerpts |
| formulary | none (roxmltree ranges discarded at the AST boundary) |
| ratex | token offsets; `ParseNode.loc` mostly `None`; lost in layout |
| mathtex | every node and glyph cluster, plus an enclosing-construct source map |
| Typst | `Span` on math properties, glyphs and shapes |

The pattern is consistent: **spans die at the first hand-written AST boundary**.
Every project that lost them had to bolt on a second mapping layer later or give
up on structural editing. mathtex is the only stack where the editor maps model
spans onto laid-out boxes directly, and that is also the cleanest editing
architecture in the survey.

## Editing architectures

Three shapes:

- **DOM as the model** (MathQuill): the tree node holds its DOM element; the
  caret is an element, geometry is `getBoundingClientRect`, hit testing compares
  X only. Works, but every edit is a DOM mutation and every measurement is the
  browser's.
- **Model offsets + DOM measurement** (MathLive): atoms with offsets, boxes in
  em, but caret points and atom bounds are measured from rendered elements.
  Caret is a flag on an atom; empty cells are `PlaceholderAtom`s; IME is a
  composition atom. Strongest partial-input behaviour of the five.
- **Export → typeset → map back** (mathtex-editor-core): editable tree with
  stable ids, export to LaTeX plus a `SpanMap`, typeset that source, map model
  spans onto IR boxes. Caret paths are serializable, empty slots are real
  objects with explicit caret placement, hit testing has a nearest-box fallback.

For a stack that owns both the model and the layout, the third design collapses
to a direct id→box map; the export/import round trip exists in mathtex only
because the engine is external.

## Fonts and metrics

- **Bundled, pre-generated metrics** (KaTeX, MathLive, ratex): deterministic and
  fast, but locked to one font family; ratex's layout cannot even accept another
  math font.
- **Font-table driven** (formulary, Typst, mathtex): MATH constants, italic
  corrections, accent attachments, variants, assemblies. Handles arbitrary
  OpenType math fonts and is the only route for "bring your own face".
- **No metrics at all** (Temml, MathQuill): whatever the browser gives you.

MATH contains font data, not layout policy: the spec explicitly defines no
algorithm. The constants still have to be wired into TeX-style rules, and every
MATH-driven stack adds fallbacks for missing or bad fields (Typst substitutes
`delimitedSubFormulaMinHeight` for Cambria's `displayOperatorMinHeight`).

## The algorithmic core

The rules are stable and documented; the work is in implementing them without a
TeX engine:

- eight spacing classes, the 8×8 matrix, bin cancellation, and the "tight" table
  in script styles (TeXbook p. 158/170; KaTeX `spacingData.ts`);
- eight style variants (D/T/S/SS × cramped) with a transition table (KaTeX
  `Style.ts:93-130`);
- script shifts, fraction/radical/limit constants, and cramped differences;
- MATH constants (a minimal set of ~40) and the three-step assembly protocol for
  stretchy delimiters (variants → reduce overlap → add extenders).

`raw/algorithm.md` carries the tables and the source for each rule.

## What a native stack should copy

1. Parse tree and layout box tree as separate, explicit stages (KaTeX,
   MathJax, Typst).
2. Atom class and spacing as a data table, applied in one pass with bin
   cancellation (KaTeX, MathLive).
3. One vlist/strut primitive for scripts, fractions, radicals, limits
   (KaTeX `makeVList`, MathLive `VBox`).
4. Byte spans on every syntax node **and** every glyph cluster, plus an
   enclosing-construct span stack (mathtex, Typst).
5. Stable node ids and a serializable caret path (mathtex).
6. Empty slots as first-class objects with placeholder rects (mathtex,
   MathLive).
7. Metrics from the font's MATH table, with documented fallbacks (Typst).
8. Error recovery that positions errors and keeps the structure around them
   (MathLive, Typst), not abort-on-first-error (KaTeX, ratex).
9. Token-list macro expansion with provenance and a hard budget (KaTeX), not
   string splicing (MathJax).
10. Declarative stretch requests resolved in a second pass (Typst).

## What to avoid

- Dropping spans at the AST boundary (four of ten stacks did; none recovered).
- Hard-coding one family's metrics (ratex); it buys parity and costs every other
  font.
- A flat display list as the only output when editing is a goal (formulary).
- A typesetting engine tied to a compiled artifact (mathtex's 3.7 MB format
  refuses a different engine build; the cost is 58k generated LOC of `unsafe`
  TeX).
- Treating MathML as a substitute for layout (pulldown-latex, Temml) when the
  host has no MathML engine.
- Treating style levels as a size multiplier; cramped variants and
  style-specific constants are the difference between correct and plausible.

## Coverage a first version needs

Ordered by dependency and real-document frequency (Writefull's 300k-arXiv dump;
KaTeX's 69 function modules; every engine's core):

1. atoms, the spacing matrix, bin cancellation, Greek and ordinary symbols;
2. superscripts/subscripts with style transitions;
3. fractions (`\frac` is the top math construct at 4.9% of all commands);
4. roots;
5. `\left...\right` and delimiters (3.1% each);
6. large operators, limits, named operators;
7. accents and bars (0.8%);
8. text and font commands (`\mathrm` 0.4%, `\text` 0.3%);
9. matrices, `cases`, `aligned`, `array`;
10. the long tail (`\over`, `\binom`, stretchy arrows, `\not`, `\pmod`,
    `\phantom`, colour, tags).

## See also

- `raw/js.md`, `raw/rust.md`, `raw/algorithm.md`: the source-grounded notes.
- [Target architecture](./architecture.md): what we build from this.
