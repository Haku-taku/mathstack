# JavaScript/TypeScript math stacks: architecture survey (raw report)

Read 2026-11-06. LOC over source directories, tests/docs excluded.

## KaTeX v0.16.22

| Fact | Value |
| --- | --- |
| Language | JavaScript + Flow (`// @flow`) |
| License | MIT |
| Core size | 19,976 JS LOC; 17,899 excluding generated `fontMetricsData.js` |
| Output | HTML+CSS, optional parallel MathML |
| Fonts | bundled KaTeX woff2/ttf; metrics generated into `src/fontMetricsData.js` |

Pipeline: `Lexer` → `Token{text, loc, noexpand}` with `SourceLocation{lexer,start,end}` (`src/Lexer.js`, `src/SourceLocation.js:5-17`); `MacroExpander` ("gullet") owns a reverse token stack + macro namespace, expands lazily (`src/MacroExpander.js:279-320`); `Parser` builds union-typed `AnyParseNode[]` (`src/parseNode.js:29`); `buildTree` walks it twice: `buildHTML` → virtual DOM (`Span`, `SymbolNode`, `SvgNode`, each with height/depth/maxFontSize) and `buildMathML` (`src/buildTree.js:34-58`, `src/domTree.js:40-140`). The parse tree is the single semantic IR; HTML and MathML are independent backends.

Symbols/macros: `src/symbols.js` mode→char/command→{font, group, replace}; behaviour registered in `src/functions/symbols*.js`. `\def`, `\gdef`, `\edef`, `\xdef`, `\let`, `\futurelet` implemented (`src/functions/def.js:7-15,87,166-202`); `\newcommand` family in `src/macros.js:145-200`; expansion substitutes `#1..#9` into token lists, bounded by `maxExpand` (`src/Settings.js:192`).

Layout: computes vertical metrics, glue and vlist offsets, expresses them as inline CSS. `buildExpression` classifies nodes (`mord/mop/mbin/...`), cancels Bin (TeX rules), inserts glue from `spacings`/`tightSpacings` (`src/buildHTML.js:104-131`, `src/spacingData.js:37-121`); mu→em via `quad/18` (`src/buildCommon.js:629-635`). `makeVList` uses pstrut + relative tops + a two-row vlist table (`buildCommon.js:537-620`). Scripts by Rules 18a-f (`functions/supsub.js:60-190`); fractions by num1/2 denom1/2, axisHeight, rule thickness (`functions/genfrac.js:35-140`); radicals with generated SVG surd + vlist (`functions/sqrt.js:35-100`); Op centered on axis, limits vlist with bigOpSpacing1..5; delimiters: small glyph → Size1-4 → `makeStackedDelim` with top/middle/bottom/extender (`src/delimiter.js:107,140,218,704`).

Metrics: `fontMetricsData.js` is generated, arrays `[depth, height, italic, skew, width]` in em (`src/fontMetrics.js:211-259`); TeX parameters hard-coded from TFM/TTF extraction (`fontMetrics.js:36-90`); extraction documented in `src/metrics/`. Horizontal advance mostly the browser's font layout.

Spans/errors: `Token.loc` and optional parse-node `loc`; `ParseError` prints position and underlines the slice (`src/ParseError.js:30-75`). Builders never consult `loc`; no output node carries a span. `throwOnError=false` renders the whole expression as an error span; unknown commands become colored text and parsing continues (`src/Parser.js:309-330,835-855`); other errors abort.

Editing: none. No incremental rendering. No in-tree benchmarks.

## MathJax v4.1.3

| Fact | Value |
| --- | --- |
| Language | TypeScript |
| License | Apache-2.0 |
| Core size | 105,768 TS LOC (TeX input 26,822; output 35,907; Mml/core 13,177) |
| Output | CHTML or SVG; `MmlNode` tree is the IR |
| Fonts | external `@mathjax/mathjax-newcm-font` |

Pipeline: `TexParser` scans the string with index `i` (`ts/input/tex/TexParser.ts:209,298`); `MapHandler` dispatches to `TokenMap`s (`MapHandler.ts:111-141`); parse methods push `MmlNode`s onto `StackItem`s (`ts/input/tex/Stack.ts`); `NodeFactory` + creators produce `MmlNode` with children/attributes (`ts/core/MmlTree/MmlNode.ts:170-185`); postfilters normalize (`cleanSubSup`, `moveLimits`, `combineRelations`) (`ts/input/tex.ts:167-176`). Output wraps nodes in `CommonWrapper` with recursive `getBBox()` (`ts/output/common/Wrapper.ts:579-640`).

Symbols/macros: `BaseMappings.ts` RegExp/Character/Command/Delimiter/Environment maps (`BaseMappings.ts:39-54,385,745`); `\def`, `\let`, `\newcommand`, `\newenvironment` in the newcommand package (`NewcommandMethods.ts:83-200`). Expansion is **string splicing**: body substituted into `parser.string`, `parser.i` reset to 0 (`BaseMethods.ts:2247-2273`); `maxMacros`/`maxBuffer` guards.

Layout: TeX 8×8 `TEXSPACE` matrix (`ts/core/MmlTree/MmlNode.ts:71-91,726-741`) → `bbox.L/R` → CHTML margins; operator dictionary contributes lspace/rspace/texClass. Fractions as CSS tables + struts (`chtml/Wrappers/mfrac.ts:137-220`); scripts via vertical-align (`msubsup.ts:334-365`); radicals `mjx-surd` + padded box; stretchy delimiters: size variants then `beg/ext/end` assembly in `mjx-stretchy-v/h` (CHTML) or outline assembly with transforms (SVG) (`chtml/Wrappers/mo.ts:250-296`, `svg/Wrappers/mo.ts:194-256`).

Metrics: `FontData` per-variant char arrays and named parameters; `getChar/getDelimiter/getSizeVariant/getStretchVariant` (`FontData.ts:1329-1400`).

Spans/errors: `Token` has no offsets (`ts/input/tex/Token.ts:24-45`); `StackItem` has `startI/stopI` and `addLatexItem` slices source into `data-latex-item` (`StackItem.ts:90-128,582-588`), used to reconstruct constructs, not for general mapping. `TexError` has no position; `compile()` wraps the whole source in an `merror`; no partial recovery.

Editing: none. `adaptiveCSS`, `Usage`, SVG `FontCache`, cached `getBBox`/break counts are perf patterns.

## Temml 0.14.0

KaTeX fork with the HTML builder removed: Lexer → MacroExpander → Parser → parse tree → `buildMathML` → MathML Core DOM/markup (`src/buildMathML.js:110-305`). 13,053 JS LOC, MIT, no bundled fonts, no metric tables. It computes almost no positions: emits MathML Core and lets the browser lay out, nudging with `lspace/rspace`, `largeop`, `movablelimits`, `fence`, `stretchy`, `minsize/maxsize`, `displaystyle/scriptlevel`; `makeRow` zeroes end lspace/rspace (`buildMathML.js:110-125`). `estimatedWidth()` in `src/stretchy.js:8-35` is a heuristic for horizontal accents in Chromium. Line breaking emulated via tables/rows, soft breaks Firefox-only (`src/linebreaking.js:1-35`). Same KaTeX span machinery, used only for one consolidation check; errors replace the whole expression.

## MathLive 0.111.4

| Fact | Value |
| --- | --- |
| Language | TypeScript |
| License | MIT |
| Core size | 64,781 TS LOC (core+atoms+editor 29,445; command tables 7,733) |
| Output | editable HTML/CSS field; MathML export |
| Fonts | KaTeX families bundled; generated `font-metrics-data.ts` |

Pipeline: tokenize → `Parser` → `Atom` tree (model) → per-atom `render()` → `Box`/`VBox` layout tree → HTML string (`parseLatex()` `src/core/parser.ts:1949-1995`; `Atom` `src/core/atom-class.ts:64-190`; `Box` `src/core/box.ts:96-320`; `VBox` `src/core/v-box.ts:229-260`). `render()` replaces the field's `innerHTML` (`render.ts:43-225`).

Symbols/macros: tables under `src/latex-commands/`; user macros come from options (`normalizeMacroDictionary`); `\def`-family recognized but the definition branch is a TODO (`src/core/parser.ts:1633-1665`); macro args scanned as strings, body recursively parsed (`parser.ts:1868-1905`).

Layout: own positions from `CHARACTER_METRICS_MAP` (not canvas/DOM measurement) (`box.ts:725-775`); 8×8 spacing with bin cancellation (`src/core/inter-box-spacing.ts:44-100`); fractions/radicals/limits as KaTeX-style vlists; delimiters sized after measuring the body (`src/atoms/leftright.ts:95-210`). Known structural limitation: inter-box spacing applied after rendering, so some dependent widths (fraction bars) rely on CSS; a three-stage `Atom → GeometryWrapper → Box` refactor is sketched (`src/core/layout-atom.ts:17-60`).

Editing: model = atom tree + `Selection{ranges}` of offsets (`model-private.ts:44-120`); caret is a flag on a Box, point from `getBoundingClientRect()` (`utils.ts:38-45`); atom bounds = union of `data-atom-id` nodes, cached; hit test walks the atom tree by cached rects with midpoint bias (`pointer-input.ts:351-575`); empty cells use `PlaceholderAtom`, missing args get implicit placeholders; IME via composition events creating a `CompositionAtom` (`src/editor/keyboard.ts:133-154`). Best partial-input behavior: parser continues, emits `ErrorAtom`s, collects errors for `validateLatex()` (`parser.ts:1103-1180`).

Spans: tokens are strings; `LatexSyntaxError` has no numeric position.

## MathQuill 0.10.1

Editing-only, MPL-2.0 (file-level copyleft), 10,672 TS LOC. Parsimmon grammar → `MQNode` command objects; `MathCommand` holds `blocks`, `MathBlock` is tree node + child sequence with DOM refs (`src/tree.ts:87-145`, `src/commands/math.ts:80-145`). No macro stage, no `\def`; unknown commands fail the parse and insertion is silently skipped (`math.ts:627-651`). Layout is CSS: hard-coded 115% font size, 90% scripts, fraction block/float/border, sqrt with positioned surd, `\left...\right` with static SVG scaled in CSS, binary operators get `padding: 0 .2em` — no spacing table, no metrics. Caret is a `<span class="mq-cursor">` with ZWSP, geometry from `getBoundingClientRect` (`src/cursor.ts:21-90,160-175`); hit test compares `clientX` to element left + half width, Y ignored (`services/mouse.ts:147-176`); empty blocks carry `mq-empty` and CSS `:after { content: 'c' }` (`editable.less:53-58`). Selection wraps DOM in `mq-selection` spans. Hidden textarea for keyboard; no composition handlers (IME browser-dependent, unverified).

## What a native stack should copy / avoid

1. Two IRs: parse AST and layout box tree (KaTeX, MathJax). MathQuill's DOM-as-model is the cautionary case.
2. Atom class and spacing as data (8×8 table), not per-command code.
3. Every box stores width/height/depth; one vlist/strut primitive serves scripts, fractions, radicals, limits.
4. Generate metrics offline into `(font, codepoint) → {width,height,depth,italic,skew}`; runtime measurement is the wrong primitive.
5. Carry source spans from lexer through AST; KaTeX drops them at build, MathJax only keeps stack-item slices.
6. Error policy per mode: whole-expression box for static rendering; continue-with-error-nodes for editing (MathLive).
7. Expand macros over token lists with a hard budget (KaTeX), not string splicing (MathJax), if spans matter.
8. Keep model offsets separate from rendered geometry (MathLive), rather than a DOM caret (MathQuill).
9. Deliberate stretchy protocol: size variants + extensible pieces (+ outline paths for the extreme).
10. Cache derived work, coalesce frames, but avoid full-document rebuilds (MathLive's innerHTML replacement is a cost).
