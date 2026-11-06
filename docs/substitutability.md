# Can this stack replace a Rust reference framework?

This page answers a narrower question than `survey.md`: for which use cases can
`mathstack` stand in for an existing Rust math stack, and what must be added
before it can. Framework facts come from `raw/rust.md`; this stack's claims name
the command or test that checks them. Anything not reproduced here is marked
`[unverified]`.

## Method

A framework is "replaceable" only for a use case both sides serve. The use
cases are:

| Use case | What the caller needs |
| --- | --- |
| Render LaTeX | parse LaTeX, lay out glyphs, paint a display list |
| Parse without layout | a LaTeX parser that returns an event/tree API |
| MathML front end | layout starting from MathML, not LaTeX |
| Editing | stable node identity, caret/selection/hit-test geometry, input operations |
| Full TeX | catcodes, registers, packages, arbitrary macros |

The matrix records the smallest thing that blocks each replacement, not a wish
list of KaTeX commands.

## Current state of this stack

| Property | State | Evidence |
| --- | --- | --- |
| LaTeX input | recursive-descent parser, spans on every node | `math-syntax`; `every_corpus_byte_is_covered_by_a_span_or_diagnostic` |
| Syntax breadth | 336/987 KaTeX probe entries; 77/77 curated constructs | `docs/completeness.md`; `conformance.rs` |
| Semantic tree | stable `NodeId` arena, `Origin`, `Span` | `math-core::Model`; M4 tests |
| Layout | TeX Appendix G rules driven by MATH constants | `math-diff --gate`, 16-formula differential |
| Fonts | any OpenType MATH face, caller-supplied bytes; `ssty` variants | `math-font`; `script_levels_take_the_fonts_alternates` |
| Output | `Layout`/`MathBox` display list; `math-svg` painter | `math-svg`; `math-editor-demo` |
| Editing | model edits, caret paths, selection, empty-slot rects, IME, undo, spellings | `math-editor`; 29 editor tests |
| Macros | token-list `\def`/`\newcommand` with `Origin::Macro`, budgets | `m3-macros-text.md` |
| Error recovery | diagnostic nodes keep the surrounding structure | `errors_recover_in_place`; `robustness.rs` |
| License | MIT OR Apache-2.0 | `Cargo.toml` |
| Assets | no baked format, no generated engine, no bundled font | workspace crates |

Performance is measured, not asserted: full reparse plus layout is 6.4 µs for an
inline formula, 258 µs for a 10×10 matrix and 4.27 ms for a 2970-node row on the
recorded machine (`docs/m5-budget.md`). The numbers are machine-dependent and
the example is the reproducible artifact.

## Framework-by-framework

### formulary 0.3.0

formulary is a MathML Core layout engine: MathML string → XML tree → AST →
flat `Layout` with no node identity (`raw/rust.md`, formulary section). It does
not parse LaTeX.

- **Render LaTeX with formulary:** replaceable. This stack owns the whole
  LaTeX path and needs no MathML intermediate. The 16-formula differential is
  the current geometry check.
- **Layout MathML with formulary:** **not replaceable**. There is no MathML
  reader or MathML output here. A host that receives MathML must keep formulary
  or add a MathML front end.
- **Editing:** formulary has none; this stack adds identity and geometry.

### pulldown-latex 0.8.0

pulldown-latex is a LaTeX pull parser returning `Event`s and an optional MathML
writer (`raw/rust.md`, pulldown-latex section). It lays nothing out.

- **Parse LaTeX into an event stream / MathML:** **not replaceable**. This stack
  exposes a tree, not `Event`s, and writes no MathML. A consumer of those APIs
  must keep pulldown-latex or a new adapter.
- **Render LaTeX:** replaceable; this stack continues past the parser into
  layout, which pulldown-latex leaves to the host.
- **Macros:** both expand token-list macros; this stack marks expanded nodes
  `Origin::Macro`, pulldown-latex keeps a call-site trace for errors.

### mathtex 0.2.0

mathtex embeds a baked XeTeX engine and exposes a retained IR plus a full
headless editor (`raw/rust.md`, mathtex section).

- **Full TeX semantics:** **not replaceable**. No catcodes, registers, TeX
  packages, or arbitrary `\def` expansion beyond token lists. A document that
  needs the LaTeX format must keep mathtex.
- **Ordinary formula rendering and editing:** replaceable for the supported
  grammar, with a much smaller artifact (no 3.1 MB format or 58k generated LOC).
  Editing geometry here is a direct id→box map, not mathtex's
  export-with-span-map round trip.
- **Exact TeX layout parity:** `[unverified]`; the differential covers 16
  formulas against formulary, not the full TeX rule set.

### ratex 0.1.15

ratex is a KaTeX port with hardcoded Computer Modern metrics and a flat display
list (`raw/rust.md`, ratex section).

- **KaTeX command parity:** **not replaceable yet**. The probe baseline is
  336/987 entries, and probe coverage is not the same as KaTeX's semantics.
- **Arbitrary math fonts:** this stack is stronger: layout is MATH-driven and
  accepts caller font bytes, while ratex cannot accept another face.
- **Editing:** ratex has none.

### Typst math

Typst is a full typesetter with its own syntax and retained frames; it is not a
LaTeX compatibility layer (`raw/rust.md`, Typst section). Neither stack replaces
the other's input language. It remains a design reference for MATH-driven
stretch and spacing, not a replacement target.

## What blocks a broader replacement

| Gap | Blocks | State | Decision |
| --- | --- | --- | --- |
| Environments (`equation`, `gather`, `smallmatrix`, matrix stars, `array`, `alignat`) | real documents | supported | implement; remaining environments land with their parser, writer and layout together |
| `\multicolumn` | spanning cells | needs a cell-span field | implement only when a host document needs it; a table with spans cannot silently degrade to equal cells |
| `CD` | commutative diagrams | not modelled | do not implement; rare, and its grid is not a table |
| Colours, `\color`/`\textcolor` | coloured documents | supported | display items carry the colour; a `\color` declaration shares the style-switch scope |
| Style/class wrappers (`\displaystyle`, `\mathop`, the eight `\math*` classes, `\mathchoice`, `\mathstrut`) | custom operators and style control | supported | `\mathop{…}\limits` is the only way to declare a custom operator |
| Top-level `\\`, `\newline`, `\par` | multi-line formulas | not modelled | defer until a host document requires it; KaTeX itself accepts them only in specific contexts `[unverified]` |
| Long-tail symbols (~420) | amsmath relations/arrows | table entries, pending | implement in batches driven by real documents, not by the probe list alone |
| `\varlim*`, decorated operator bodies | amsmath limit operators | supported | a decorated `lim` is a `Function` under a decoration inside an `Op` class |
| Full macro semantics (`\let`, `\edef`, `\expandafter`) | TeX packages | deliberately out of scope | do not implement; a token-list expander is the boundary |
| MathML in/out | formulary/pulldown-latex consumers | not implemented | do not implement; these are a different front/back end, not part of the LaTeX rendering path |
| Event API / MathML writer | pulldown-latex consumers | not exposed | do not implement without a named consumer; an adapter can be added without changing the model |

## Conclusion

For **embedded LaTeX math rendering and editing with a caller-supplied OpenType
MATH font**, this stack can replace formulary's layout half, pulldown-latex's
render path, ratex for the supported command subset, and mathtex for ordinary
formulas without a TeX installation. It cannot yet replace:

- formulary for MathML input;
- pulldown-latex for an `Event` API or MathML output;
- mathtex for full TeX semantics;
- ratex for complete KaTeX parity.

Those four exclusions are capability boundaries, not missing polish; closing
them would require a MathML front end, an event API, a TeX engine, or the full
KaTeX command set respectively.

The decisions above order the remaining work by whether it blocks the LaTeX
rendering path: environments, colours and style wrappers do, so they come
first; MathML, an event API and TeX macros are separate surfaces and stay out.
