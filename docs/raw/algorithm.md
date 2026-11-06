# Math typesetting: algorithmic core (raw report)

Revisions: TeXbook Appendix G transcription + `tex.web` + plain.tex; KaTeX main 2026-11-03 (`src/spacingData.ts`); Typst 2026-10-28; MathQuill 0.10.1; MathLive 2026-10-31; formulary 0.3.0; OpenType 1.9.1 MATH spec.

## 1. TeX math layout

### 1.1 Atom classes

Thirteen atom kinds (TeXbook p. 158); eight determine spacing:

| Class | TeX primitive | Typical |
| --- | --- | --- |
| Ord | `\mathord` | letters, digits, `\alpha` |
| Op | `\mathop` | `\sum`, `\int`, `\sin`, `\lim` |
| Bin | `\mathbin` | `+`, `-`, `\times`, `\cup` |
| Rel | `\mathrel` | `=`, `<`, `\to`, `\subset` |
| Open | `\mathopen` | `(`, `[`, `\{` |
| Close | `\mathclose` | `)`, `]`, `\}` |
| Punct | `\mathpunct` | `,`, `;` |
| Inner | `\mathinner` | `\frac`, `\left...\right`, matrices |

### 1.2 Inter-atom spacing

`0` none, `1` thin 3mu, `2` medium 4mu, `3` thick 5mu; `*` impossible; parentheses = display/text only (suppressed in S/SS). Rows = left atom, columns = right atom.

| | Ord | Op | Bin | Rel | Open | Close | Punct | Inner |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Ord | 0 | 1 | (2) | (3) | 0 | 0 | 0 | (1) |
| Op | 1 | 1 | * | (3) | 0 | 0 | 0 | (1) |
| Bin | (2) | (2) | * | * | (2) | * | * | (2) |
| Rel | (3) | (3) | * | 0 | (3) | 0 | 0 | (3) |
| Open | 0 | 0 | * | 0 | 0 | 0 | 0 | 0 |
| Close | 0 | 1 | (2) | (3) | 0 | 0 | 0 | (1) |
| Punct | (1) | (1) | * | (1) | (1) | (1) | (1) | (1) |
| Inner | (1) | 1 | (2) | (3) | (1) | 0 | (1) | (1) |

TeXbook p. 170 + Appendix G Rule 20; KaTeX `src/spacingData.ts:37-88`, tight variants `90-112`. TeX's internal encoding distinguishes conditional thin (1) from unconditional thin (2), conditional medium (3) and thick (4) (`tex.web:15062-15080`) — why Punct spacing vanishes in scripts while Ord–Op thin stays.

Operational rules:
- Bin cancellation first: Bin at start, after Bin/Op/Rel/Open/Punct, or before Rel/Close/Punct becomes Ord; trailing Bin becomes Ord (App. G Rules 5–6; KaTeX `src/buildHTML.ts:106-117`).
- Spacing glue goes between adjacent atoms, not around lists; `\left`/`\right` are Open/Close (Rule 19).
- `1mu = quad/18`; thin/medium/thick = 3/4/5mu (`plain.tex:373-375`; KaTeX `fontMetrics.ts:258`).
- Script/scriptscript use the tight table (`Style.isTight()` = `size >= 2`, KaTeX `Style.ts:72-74`).

### 1.3 Style levels, transitions, cramped

Styles `D > D' > T > T' > S > S' > SS > SS'`; prime = cramped (TeXbook ch. 17; App. G preamble). KaTeX transition arrays `src/Style.ts:93-130`:

| Current | sup | sub | frac num | frac den | cramped |
| --- | --- | --- | --- | --- | --- |
| D | S | Sc | T | Tc | Dc |
| Dc | Sc | Sc | Tc | Tc | Dc |
| T | S | Sc | S | Sc | Tc |
| Tc | Sc | Sc | Sc | Sc | Tc |
| S | SS | SSc | SS | SSc | Sc |
| Sc | SSc | SSc | SSc | SSc | Sc |
| SS | SS | SSc | SS | SSc | SSc |
| SSc | SSc | SSc | SSc | SSc | SSc |

Script geometry (Rules 18a–18f):
- Character-box nucleus: u = v = 0; else u = h − σ18, v = d + σ19.
- Subscript only: shift down max(v, σ16, h(y) − (4/5)|σ5|), then `\scriptspace`.
- Superscript: u ← max(u, p, d(x) + (1/4)|σ5|), p = σ13 (D), σ15 (C′), else σ14; add `\scriptspace`.
- Both: v ← max(v, σ17); if gap < 4ξ8, raise sup to 4/5 x-height then push sub down; italic correction.
- `\scriptspace` = 0.5pt (`plain.tex:347`; KaTeX `supsub.ts:112-115`).

Cramped differs at: superscript shift (`...Cramped`/σ15); `\overline`, `\sqrt`, `\mathaccent` nucleus in C′ (Rules 9, 11, 12); fraction denominator T′/C′ (Rule 15a); `\underline` uses C (Rule 10). Typst `fraction.rs:30-49`; formulary `layout.rs:1039-1043`.

### 1.4 Font parameters

22 σ from family 2, 13 ξ from family 3, per size. Key mapping (TeXbook p. 441):

| TeX | Meaning | MATH analogue |
| --- | --- | --- |
| σ5 x-height | 4/5 checks | OS/2 sxHeight |
| σ6 quad | 18mu | font size |
| σ8–σ10 num1/2/3 | numerator shift/clearance | fractionNumeratorShiftUp(Display), gaps |
| σ11–σ12 denom1/2 | denominator shift | fractionDenominatorShiftDown(Display) |
| σ13–σ15 sup1/2/3 | minimum sup shift | superscriptShiftUp, ...Cramped |
| σ16–σ17 sub1/2 | minimum sub shift | subscriptShiftDown |
| σ18–σ19 supDrop/subDrop | box nucleus | superscriptBaselineDropMax, subscriptBaselineDropMin |
| σ20–σ21 delim1/2 | generalized fraction delimiters | delimitedSubFormulaMinHeight |
| σ22 axisHeight | fraction bar, operator centering | axisHeight |
| ξ8 rule thickness | fraction/radical rules | fractionRuleThickness, radicalRuleThickness |
| ξ9–ξ13 big-op spacing | limits | upper/lowerLimitGapMin, ...RiseMin, ...DropMin |
| `\scriptspace` | after scripts | spaceAfterScript |
| `\delimiterfactor=901`, `\delimitershortfall=5pt`, `\nulldelimiterspace=1.2pt` | \left/\right sizing | engine policy |

Defaults `plain.tex:327,345-347`; KaTeX table `fontMetrics.ts:41-75`.

## 2. OpenType MATH

- MathConstants — 56 fields: 5 integers (scriptPercentScaleDown, scriptScriptPercentScaleDown, delimitedSubFormulaMinHeight, displayOperatorMinHeight, radicalDegreeBottomRaisePercent) and 51 MathValueRecords (design units + optional device correction).
- MathGlyphInfo — italics correction, top-accent attachment, extended-shape coverage, MathKernInfo (4 corner staircases).
- MathVariants — minConnectorOverlap, vertical/horizontal coverage, per-glyph MathGlyphConstruction: variants (variantGlyph, advanceMeasurement) + optional GlyphAssembly (assembly italics correction; parts with glyphID, start/end connector, fullAdvance, EXTENDER flag). Spec: variants first, then assembly.

The spec defines no layout algorithm. Minimal constant set for a first version:

| Group | Constants |
| --- | --- |
| Scaling | scriptPercentScaleDown, scriptScriptPercentScaleDown |
| Axis/ops | axisHeight, displayOperatorMinHeight |
| Scripts | superscriptShiftUp(+Cramped), superscriptBottomMin, superscriptBaselineDropMax, superscriptBottomMaxWithSubscript, subscriptShiftDown, subscriptTopMax, subscriptBaselineDropMin, subSuperscriptGapMin, spaceAfterScript |
| Limits | upper/lowerLimitGapMin, upper/lowerLimitBaselineRiseMin/DropMin; stretchStack* shifts and gaps |
| Stacks | stackTopShiftUp(+Display), stackBottomShiftDown(+Display), stackGapMin(+Display) |
| Fractions | numerator/denominator shift up/down (+Display), numerator/denominator gap min (+Display), fractionRuleThickness |
| Radicals | radical (DisplayStyle) verticalGap, ruleThickness, extraAscender, kernBefore/AfterDegree, degreeBottomRaisePercent |
| Accents/bars | accentBaseHeight, flattenedAccentBaseHeight, over/underbar gap, rule thickness, extra ascender/descender |

Formulary exposes all 51 indices (`src/math.rs:88-180`); Typst defines the subset (`typst-library/src/text/font/metrics.rs:140-197`) and substitutes delimitedSubFormulaMinHeight for displayOperatorMinHeight on Cambria Math (`metrics.rs:229-233`).

## 3. Stretchy delimiters and large operators

- TeX: successor chain in small/large families per style; extensible → vbox of pieces; else tallest or `\nulldelimiterspace`. Rule 19 target: δ = max(h−a, d+a); height ≥ max(floor(δ/500)·901, 2δ−5pt); centered on axis. KaTeX `src/delimiter.ts:807-830`.
- KaTeX: no MATH; Size1–4 fonts, stacking from Size4 pieces, repeat = ceil((total−min)/(middleFactor·repeatHeight)), 0.008em overlap, middle replaced by SVG (`delimiter.ts:218-430,604`); `\big` = [0,1.2,1.8,2.4,3.0]em.
- formulary: variants smallest-first, else assembly with uniform overlap `(sum−target)/joints` clamped to [minConnectorOverlap, maxOverlap], ≤32 repeats (`font.rs:289-359`).
- Typst: spec procedure — extenders removed + max overlap, reduce overlap toward min, add one extender (`glyph.rs:268-313,565-660`); warns if connector < minConnectorOverlap.

Large operators: TeX Rule 13 successor in D, axis centering, italic correction in width iff limits or no subscript; limits via ξ9–ξ13. MATH portable rule: display stretches to displayOperatorMinHeight, text uses base glyph (Typst `glyph.rs:442-460`; formulary `layout.rs:1652-1660,1741-1762`).

## 4. Editing geometry

- MathQuill: DOM is the layout engine. Caret = `<span class="mq-cursor">` with ZWSP inserted between node elements (`src/cursor.ts:28-32,62-120`), geometry from getBoundingClientRect (`cursor.ts:160-175`). Empty slots are empty blocks with `mq-empty`; CSS width via hidden `c` (`math.ts:656-665`, `editable.less:53-58`, `math.less:92-100`). Hit test: `event.target` → nearest node → compare clientX to left + half offsetWidth; Y ignored (`services/mouse.ts:147-176`); vertical movement by keyboard only (`cursor.ts:122-152`).
- MathLive: own box model in em (`src/core/box.ts:90-180`), but caret geometry measured from DOM. Caret is an atom flag emitting `ML__caret` (`render.ts:106-122`); point from getBoundingClientRect (`utils.ts:27-48`). Atom bounds = union of `data-atom-id` nodes, cached (`utils.ts:141-182`). Flat atom array + `offsetOf`; `PlaceholderAtom` per empty cell (`array.ts:79-80,461-462`). Hit test: field rect ±8px, nearest atom by squared distance, arrays pick row by vertical distance, midpoint bias (`pointer-input.ts:348-368,375-420,510-575`); `getElementInfo(offset)` returns bounds/depth/style (`utils.ts:308-330`).

What a layout engine must expose (design implication): stable node/slot ids; offset ordering; per-node box (x, baseline y, width, ascent, depth) + screen transform; per-slot rects incl. empty; hit_test → offset with midpoint bias, tolerance, array row selection; caret_rect from the box tree; per-branch selection rects; style/scriptLevel/cramped per node; scroll/scale transform; rendered-node back-reference.

## 5. Real-world command coverage

- Writefull, 300k arXiv papers (all LaTeX commands, not math-only): `\frac` 4.9%, `\left`/`\right` 3.1% each, `\alpha` 1.6%, `\mu` 1.6%, `\pi` 1.4%, `\sum` 1.1%, `\sigma` 1.1%, `\int` 0.9%, `\partial` 0.9%, `\sqrt` 0.8%, `\bar` 0.8%, `\over` 0.9%, `\mathrm` 0.4%, `\prime` 0.4%, `\text` 0.3%. `\begin`/`\end` 6% mix math and non-math.
- KaTeX: 69 defineFunction modules; 736-line supported table. MathJax base: frac, sqrt, root, over, choose, left/right/middle, mathop/bin/rel, text, accents, matrix, array, begin/end. Typst: bottleneck is constructs, not symbols. Expressions follow a Zipf-like law (arXiv:2002.02712).

First-version ordering: 1) atoms + 8×8 spacing + bin cancellation + symbols; 2) scripts; 3) fractions (+tfrac/dfrac/genfrac); 4) roots; 5) \left...\right + delimiters; 6) large operators/limits/named operators; 7) accents/bars/primes; 8) text/font commands; 9) matrices/cases/aligned/array [speculative rank]; 10) long tail (\over, \binom, stretchy arrows, \not, \pmod, \phantom, \cancel, color, tags, \substack).

## Minimum viable layout core

Ordered by dependency:
- Box model + font metrics: width/ascent/depth, h/v lists, style, script level, cramped, unitsPerEm, x-height, quad, italic correction, ink bounds.
- MATH reader with fallbacks; TeX-like defaults when fields are absent (Typst `metrics.rs:325-380`).
- Atom classifier + spacing pass (bin cancellation, tight variants by style).
- Style engine: D/T/S/SS + primes, transition table, scale percentages, ssty/flac.
- Symbol layout: advance, italic correction, class, ssty alternates, axis centering for Op.
- Scripts: u/v, minima, gap, scriptspace, limits vs scripts.
- Fractions: shifts/gaps, rule on axis, display variants, bar-less \atop/\binom.
- Radicals: cramped radicand, gap + rule, stretch, degree kerns.
- Delimiters: variants then assembly, \left/\right target, null delimiters.
- Large operators: display successor/min height, axis centering, italic correction, limits.
- Accents/bars: top-accent attachment, base height, flattening, stretchy horizontal.
- Text/operator names: text-mode runs, \operatorname spacing class.
- Tables/matrices/aligned: column widths, row baselines, alignment points, nests, delimiters.
- Editor geometry (if needed): ids, per-node/slot rects, offset mapping, point query, selection rects, transforms.

Out of scope first version: macro expansion, color, tags/references, MathML semantics, line breaking inside formulas, full \fontdimen emulation.
