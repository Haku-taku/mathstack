# Rust math stacks: architecture survey (raw report)

Sources read: local cargo registry for formulary 0.3.0, pulldown-latex 0.8.0, mathtex 0.2.0 / engine / IR / font 0.2.0 / editor-core 0.4.1, ratex 0.1.15 (ratex-layout downloaded from crates.io); Typst 0.15.1 (`typst-library`, `typst-layout`, `typst-syntax`). LOC excludes tests and examples.

formulary is a MathML Core stack, not a LaTeX front end; pulldown-latex stops at MathML; only mathtex, ratex and Typst lay out glyphs.

## formulary 0.3.0

- Size/license: 9 files, ~4,506 production LOC, MIT OR Apache-2.0. Core: `layout.rs` 71 KB, `parse.rs` 24 KB, `font.rs` 19 KB.
- Pipeline: MathML string → `roxmltree` XML tree → typed `MathRoot`/`Node` AST (`src/ast.rs:238`) → recursive layout → flat `Layout { width, ascent, descent, items }` (`src/layout.rs:34`). `Item` = `Glyph { id, x, y, size, advance, color, mirrored } | Rule | Background` (`src/layout.rs:49`). No node identity survives.
- Macros/symbols/errors: no TeX macros. Operator spacing/properties from a generated 1,155-entry MathML Core operator dictionary (`src/opdict.rs:1-16`). Lenient parsing: unknown/malformed elements become `mrow`, recorded in `MathRoot::warnings` (`src/parse.rs:84-98`, `ast.rs:228-236`); warnings carry no source position.
- Fonts: borrows `read-fonts`/`skrifa` and `harfrust`; writes its own total MATH-table reader (`src/math.rs:1-9`): constants, italics correction, top-accent attachment, MathKern, variants, assemblies. Device deltas ignored. Extended glyphs: pre-drawn variants smallest-first, then extender stacking with uniform overlap, max 32 repetitions (`src/font.rs:289-360`).
- Layout: MathML Core, not Appendix G. `displaystyle` + `script_level` + `cramped`; scale factors from MATH with 0.71/0.5041 fallbacks (`layout.rs:105-163`). Operator spacing from dictionary lspace/rspace in 1/18 em. Fractions/radicals/scripts use MATH shifts and constants; MathKern cut-ins; `MathBox` internal, flattened at the end.
- Spans: none at any stage; `roxmltree` byte ranges discarded in `parse_node`.
- Editing: none.
- Lessons: tiny embeddable core with caller-supplied font bytes; self-contained MATH reader; no spans is fatal for an editor.

## pulldown-latex 0.8.0

- Size/license: 12 files, ~7,039 production LOC, MIT. `primitives.rs` 96 KB, `mathml.rs` 63 KB, `event.rs` 30 KB.
- Pipeline: LaTeX `&str` → pull iterator of `Result<Event, ParserError>`; optional MathML writer. No AST, no layout. `Event` = `Content | Begin | End | Visual | Script | Space | StateChange | EnvironmentFlow` (`src/event.rs:57-95`). Writer tracks environment/style stack and error-recovery flag (`src/mathml.rs:20-55`).
- Macros: `MacroContext` with `Macro`/`Alias`/`Command` definitions, parameter text with prefix/suffix and `#n`, `\def`, `\newcommand`/`\renewcommand`/`\providecommand`; `\edef` unsupported. Bump-arena expansion, 64 levels, 1 MiB cap (`src/parser.rs:30-31, 244`).
- Symbols: classification predicates in `src/parser/tables.rs`; command→codepoint mappings in `primitives.rs`, not one table.
- Errors: `Err(ParserError)` with excerpt and macro call-site trace; no error AST. MathML writer catches each error, emits `<merror>`, keeps writing (`mathml.rs:578-590`). No line/column or byte-range API.
- Spans: none in events; `SpanStack` tracks byte pointers only for error excerpts (`parser.rs:452-514`).
- Editing: none.
- Lessons: compact renderer-agnostic event API; unusually good macro error trace; delegates all positioning to MathML/CSS; spans must be added to events or no editor is possible.

## mathtex 0.2.0 (engine / IR / font / editor-core 0.4.1)

- Size/license: engine 3,011 LOC, IR 835, font 1,229, editor 6,086; plus 58,345 LOC of generated XeTeX (3.1 MB crate). Wrappers MIT OR Apache-2.0; generated crate carries TeX/eTeX/XeTeX notices.
- Pipeline: baked XeTeX `Format` + LaTeX → real XeTeX engine → captured node tree → `lower()` → retained IR with dense `NodeId`. `Fragment { root, nodes: Vec<LayoutNode>, source_map, metadata, surface }` (`mathtex-ir/src/lib.rs:7-20`); `LayoutNode { id, origin, width, height, depth, primary_source, kind }`, kinds `Box(H|V)`, `GlyphRun`, `Rule`, `Glue`, `Kern`. `flatten()` for painting; tree retained.
- Macros/symbols: it is TeX/XeTeX — `\def`, `\newcommand`, catcodes, registers, whole LaTeX format. Format baked by INITEX from `latex.ltx` + preamble (`format.rs:96-145`). Errors fatal to the run, with `TypesetError::Tex { message, line, span }`, byte span clipped to caller input; operation budget, node cap, sandboxed I/O.
- Fonts: `ttf-parser` + `rustybuzz`; exposes MATH constants, italics correction, MathKern, variants, assemblies, min overlap, top-accent attachment; mapped to TeX math parameters. Stretching is XeTeX's own code.
- Layout: full Appendix G, four styles + cramped, TeX shifts and fraction/radical rules; some parameters MATH-derived, procedural rules hardcoded.
- Spans: best in class. `LayoutNode.primary_source`, enclosing construct spans in `SourceMap.entries`, per-glyph `cluster: Option<ByteSpan>`; ligatures merge source chars' spans; errors carry line + byte span.
- Editing: full headless structural editor. Stable slotmap ids, `Seq` runs per slot; exports LaTeX plus `SpanMap`; typesets then maps model spans onto IR boxes; `render()` returns caret rect, selection rects, empty-slot placeholder rects, menu anchor, host-object rects; `hit_test()` smallest containing span with nearest fallback; `CaretPath` serializable; `EmptySlotCaret::{Start,Center,Inset}`.
- Costs: huge generated engine, `unsafe`, formats tied to engine build, baking needs TeX Live. "A few milliseconds" per formula.
- Lesson: the export-with-spans → typeset → map-back design is the cleanest editor architecture in the survey.

## ratex 0.1.15

- Size/license: lexer 386, parser 12,173, types 565, font 4,828, layout 10,459 (~28.4k), MIT. `layout/src/engine.rs` 7,356 lines.
- Pipeline: LaTeX → lexer `Token { text, loc, .. }` → KaTeX-style macro expander → `ParseNode` AST → `LayoutBox` tree → `DisplayList` (flat, absolute coordinates).
- AST box model: `LayoutBox { width, height, depth, content, color }`; `BoxContent` has HBox/VBox/Glyph/GlyphRun/Rule/Kern/Fraction/SupSub/Radical/OpLimits/Accent/LeftRight/Array/SvgPath/Framed/RaiseBox. No node ids, no spans; flattened in `to_display.rs`.
- Macros/symbols: KaTeX-shaped expander; `\def/\gdef/\edef/\xdef`, `\let`, `\futurelet`, `\newcommand`; expansion budget/depth limits; delimited macros simplified. 1,883 symbol entries generated from KaTeX `symbols.ts`, ~2k metric rows. Errors abort at first failure; `ParseError { message, loc }`; no recovery.
- Fonts: does not parse fonts. Metrics hardcoded from KaTeX `fontMetrics.ts` in em, size tables from Computer Modern; outlines loaded by renderers via `ab_glyph`.
- Layout: KaTeX port. 8×8 spacing matrix with tight variant, binary cancellation, four styles + cramped, size multipliers 1.0/0.7/0.5, fractions Rule 15d, radicals Size1-4, delimiters size1-4 then stacked pieces/KaTeX SVG paths. All constants compile-time, not MATH.
- Spans: token locations exist; `ParseNode.loc` mostly `None` (5 of 135); lost in layout.
- Editing: none.
- Lessons: hardcoded metrics buy KaTeX parity and cost arbitrary fonts; `LayoutBox → DisplayList` is a clean renderer boundary; identity must be added for editing.

## Typst 0.15.1 math layout

- Size/license: Apache-2.0. `typst-library/src/math` 5,963 LOC; `typst-layout/src/math` 3,815 LOC; math parsing ~500 LOC in `typst-syntax/src/parser.rs`.
- Pipeline: source → syntax tree → content realization → `MathResolver` builds typed IR (`library/src/math/ir/resolve.rs:40-150`) → `MathContext` lays IR into retained nested `Frame`s (`layout/src/math/mod.rs:364-430`). `Frame` holds `Arc<LazyHash<Vec<(Point, FrameItem)>>>`; `FrameItem::Group` nests.
- IR: `MathItem::{Component, Spacing, Space, Tag}`; `MathComponent { kind, props, styles }`; `MathKind` includes Group, Multiline, Radical, Fenced, Fraction, SkewedFraction, Table, Scripts, Accent, Cancel, Line, Primes, Text, Number, Glyph, Box, Mathml, External. `MathProperties` holds limits, class, size, cramped, ignorant, spaced, lspace/rspace, align_form_infix, **span**.
- Naming: `MathContext` is in the layout crate; `MathRun = Vec<MathFragment>` (one cell between alignment points); `MathFragment = Glyph | Frame | Space | Tag`; no `MathBox` in 0.15.1 (older releases had one); boxed fragment is `FrameFragment`.
- Macros/symbols/errors: no TeX macro language; symbols/functions from the `codex` crate; classes from `unicode_math_class`. Parser recovers and keeps going (`expected()` inserts an error node, `unexpected()` converts and eats); depth cap 256; errors positioned by syntax spans.
- Fonts: `ttf-parser` + `rustybuzz`; MATH constants cached once; italic correction, accent attachment, extended-shape coverage, MathKern direct lookups. Stretch: variants, then assembly with repeated extenders, growable overlap ratio, MAX_REPEATS 1024; `ssty`/`flac` fallbacks.
- Layout: MATH-driven. Fractions read numerator/denominator shifts, rule thickness, axis height with display variants; radicals read gap/rule/extra ascender/degree kerns; scripts read shift up/down, maxima, gap min, space after script, MathKern; accents use accent base height / flattened base height. Styles Display/Text/Script/ScriptScript + cramped; size scale-downs from MATH. Spacing is a class-pair resolve pass (thin 1/6 em, medium 2/9, thick 5/18; script sizes suppress), not a raw Appendix G table.
- Alignment: `&` becomes an alignment point; `process_group` splits into `AlignedRow`s of `MathRun` cells; cells alternate right/left of each point.
- Spans: in math props, glyphs, shapes; positioned errors.
- Editing: none (typesetter); `typst-ide` does source completion only.
- Lessons: best reference for a resolve pass encoding spacing/style per item, declarative stretch requests resolved in a second pass, MATH-driven variants/assembly with flac/ssty.

## Comparison

| | Tree identity | Spans | Editing geometry | Font parsing | Core LOC | License | Assets |
| --- | --- | --- | --- | --- | --- | --- | --- |
| formulary 0.3.0 | none (flat items) | none | none | own MATH reader; read-fonts/skrifa/harfrust | ~4.5k | MIT/Apache-2.0 | font bytes |
| pulldown-latex 0.8.0 | none (events) | none | none | none | ~7.0k | MIT | none |
| mathtex 0.2.0 | dense NodeId IR + editor slotmap | full node + cluster spans | full caret/selection/empty slots/hit test | ttf-parser/rustybuzz + MATH accessors | 5.1k + 6.1k editor + 58.3k generated | MIT/Apache + TeX notices | baked format + fonts |
| ratex 0.1.15 | LayoutBox, no ids | token offsets, mostly lost | none | none; hardcoded KaTeX tables | ~28.4k | MIT | 192 KB tables |
| Typst 0.15.1 | retained Frame, Tag/Span identity | Span on props/glyphs/shapes | none | ttf-parser/rustybuzz + MATH cache | ~9.8k math + ~0.5k parser | Apache-2.0 | fonts + codex |

## What a native stack should copy / avoid

Copy:
- Separate parse → semantic/layout IR → box/frame → render with explicit serializable types.
- Byte spans on every syntax node and glyph cluster; keep an enclosing-construct span stack.
- For editing, export-with-span-map → typeset → map boxes back (mathtex-editor-core); keep the editable tree independent of layout.
- Drive dimensions from MATH: constants, italics correction, accent attachment, MathKern, variants, assemblies; a second resolve pass for stretch.
- Empty slots as real model objects with placeholder spans and explicit caret placement.
- Positioned, local error recovery (Typst error nodes, pulldown-latex `<merror>` continuation).

Avoid:
- Losing spans at the first AST boundary.
- Hardcoding one font family's metrics into layout (ratex).
- A flat list as the only output when editing is a goal.
- A format tied to a compiled engine build unless in-place regeneration is affordable; do not copy 58k LOC of generated XeTeX unless full TeX semantics are mandatory.
- Treating MathML output as a substitute for layout.
- Treating style levels as just a size multiplier.
