# M1 differential against formulary

The M1 exit criterion has two halves: the corpus lays out without panics, and
the divergences from a mature MATH-driven stack are recorded. This page is the
second half. `crates/math-diff` lays each formula out with this stack and with
formulary 0.3.0 at 10pt on the same font bytes and prints extents and item
lists.

```sh
cargo run -p math-diff -- /usr/share/fonts/wps-office/DejaVuMathTeXGyre.ttf --markdown
```

Font: DejaVu Math TeX Gyre (1000 upem, MATH table). Each case pairs a hand-built
`Node` tree with the MathML string that describes the same construct; formulary
lays out the MathML. Display cases use `display="block"` and `Style::DISPLAY`.

| Formula | Ours width/ascent/descent | formulary | Δ width | Δ ascent | Δ descent |
| --- | --- | --- | --- | --- | --- |
| row spacing | 45.830 / 7.600 / 0.390 | 45.830 / 7.600 / 0.390 | +0.000 | +0.000 | +0.000 |
| superscript | 13.220 / 9.746 / 0.000 | 13.220 / 9.746 / -0.000 | +0.000 | +0.000 | +0.000 |
| subscript | 10.446 / 5.340 / 2.770 | 10.446 / 5.330 / 2.770 | +0.000 | +0.010 | +0.000 |
| both scripts | 13.220 / 10.088 / 4.344 | 13.220 / 9.746 / 3.326 | +0.000 | +0.342 | +1.018 |
| nested script | 18.805 / 11.681 / 0.000 | 18.805 / 12.353 / -0.000 | -0.000 | -0.672 | +0.000 |
| fraction | 6.256 / 8.414 / 4.722 | 6.256 / 8.406 / 4.722 | -0.000 | +0.008 | -0.000 |
| fraction row | 18.440 / 9.966 / 4.778 | 21.996 / 9.966 / 4.778 | -3.556 | +0.000 | -0.000 |
| fraction scripts | 12.537 / 11.901 / 4.898 | 12.537 / 11.901 / 5.562 | -0.000 | +0.000 | -0.664 |
| radical | 13.850 / 8.665 / 1.035 | 13.850 / 8.665 / 1.035 | -0.000 | +0.000 | +0.000 |
| indexed radical | 14.255 / 8.686 / 1.035 | 14.255 / 8.686 / 1.035 | -0.000 | +0.000 | +0.000 |
| radical of fraction | 13.456 / 12.596 / 6.464 | 13.456 / 12.592 / 6.468 | +0.000 | +0.004 | -0.004 |
| parenthesized fraction | 16.356 / 10.360 / 4.860 | 16.356 / 10.360 / 4.860 | +0.000 | +0.000 | +0.000 |
| bracketed square | 22.660 / 10.330 / 4.830 | 22.660 / 10.330 / 4.830 | +0.000 | +0.000 | +0.000 |
| sum with limits | 15.560 / 15.534 / 11.706 | 23.338 / 15.534 / 11.706 | -7.778 | +0.000 | -0.000 |
| integral with side scripts | 13.990 / 13.386 / 6.212 | 15.793 / 13.386 / 6.212 | -1.803 | +0.000 | +0.000 |
| matrix 2x2 | 18.990 / 7.600 / 12.880 | 29.990 / 15.680 / 10.180 | -11.000 | -8.080 | +2.700 |
| display style switch | 32.394 / 7.600 / 0.390 | 32.394 / 7.600 / 0.390 | +0.000 | +0.000 | +0.000 |
| mathop under limits | 7.390 / 7.600 / 7.192 | 11.932 / 7.600 / 5.612 | -4.542 | +0.000 | +1.580 |
| combined | 120.789 / 13.950 / 6.370 | 120.789 / 13.950 / 6.370 | +0.000 | +0.000 | +0.000 |

## What the deltas mean

Eleven of nineteen formulas agree within 0.01pt, including both fence cases,
the radical over a fraction (which needs a stretched surd) and a style switch.

- **Script alternates (`ssty`) were missing, now fixed.** Before the fix, the
  script-level glyphs were the base forms: script digits and letters had the
  wrong advance, so widths were off by up to 8.9pt (`sum with limits`) and by
  0.4–0.7pt wherever a script stood alone. `math-font` now reads the `ssty`
  alternate subtables from GSUB and `layout_atom` applies them at script depth
  1 and 2. After the fix every width that only depends on glyph choice matches.
- **Tight spacing in script-level rows (TeX rule, intended).** `fraction row`
  is 3.556 narrow: two Ord–Bin medium gaps of 4/18 em at 8pt, which the tight
  table suppresses and MathML Core keeps. `sum with limits` is 7.778 narrow:
  the same two thick gaps in `i=1` (4.444) plus 3.338 of operator lspace/rspace
  that MathML Core bakes into the limits construct and TeX applies through the
  row's spacing table.
- **Script shifts (TeX Rule 18 vs MathML Core).** `both scripts`
  (+0.342 ascent, +1.018 descent), `fraction scripts` (−0.664 descent) and
  `nested script` (−0.672 ascent) differ because we use Rule 18e's 4ξ8
  clearance and the 0.8·x-height minimum, while MathML Core uses
  SuperscriptBottomMin / SubSuperscriptGapMin. The glyph positions agree; only
  the shifts differ.
- **Large-operator scripts.** `integral with side scripts` now matches
  formulary's ascent and descent: a large operator is not a character box, so
  Rule 18a shifts both scripts by the stretched glyph's height and depth. The
  remaining width delta is the operator's lspace/rspace, which the row's
  spacing table owns. Both stacks pick the same variant (integral.v2).
- **Tables pad inside the cell (MathML rule).** `matrix 2x2` is 11pt narrower
  and 8.08pt shallower on ascent: formulary's `mtable` adds 0.4em padding
  around every cell and a row strut, while this stack applies TeX's
  `rraycolsep`/`rraystretch` gaps between cells and adds no outer padding.
  The left glyph offset differs by exactly 4pt; the row pitch is 12.74pt
  against 12.93pt, so the rest is padding and a small row strut, not a
  different line-placement rule.
- **Under scripts reserve side space (MathML rule).** `mathop under limits` is
  4.542pt wider in formulary and 1.58pt deeper: MathML's `munder` keeps
  lspace/rspace around the under script and places it below the operator with
  its own gap, while TeX's Rule 18a centres the script under the operator with
  the operator-script clearance. Both place the script's centre on the
  operator's centre.
- **A style switch does not move anything.** `display style switch` matches
  exactly: `\displaystyle` on a group changes the cramped bit and the level's
  constants, and in a formula whose glyphs are already text-sized neither stack
  shifts a glyph or changes a width.

## Limits of this check

- One font. Constants and variant coverage are font-specific; a font without
  `ssty` or with different limit constants will produce different deltas.
- Metrics only. Glyph positions are compared when the item lists are inspected,
  but paths are not; the SVG goldens will cover that.
- The corpus is hand-paired with MathML. M2's parser removes the pairing step
  and lets the same report run from source text.
