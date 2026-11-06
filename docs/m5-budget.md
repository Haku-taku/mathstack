# M5, part 3: the per-edit budget

A keystroke changes the source, so the host reparses the formula and lays it out
again. The M5 exit criterion is that this fits an interactive frame. The
measurement tool is `math-diff`'s `edit_budget` example:

```sh
cargo run --release -p math-diff --example edit_budget -- \
  /usr/share/fonts/wps-office/DejaVuMathTeXGyre.ttf
```

It parses and lays out each case 1000 times and reports the mean. Two numbers
are printed: the whole per-edit pipeline, and layout alone on the model parsed
before the loop, so the split between parse and layout is visible. `frame` is
the pipeline's share of a 16.7 ms frame (60 Hz).

## Measurement

Release build, DejaVu Math TeX Gyre at 16 units, Intel Core Ultra 7 270K Plus,
one run (a second run differed by under 2%):

| Case | Nodes | Bytes | Parse+layout | Layout only | Frame |
| --- | --- | --- | --- | --- | --- |
| Inline formula `\frac{-b\pm\sqrt{b^2-4ac}}{2a}` | 18 | 30 | 6.4 µs | 5.3 µs | 0.04% |
| Display sum `\sum_{i=1}^{n} i^2 = …` | 29 | 43 | 10.4 µs | 9.3 µs | 0.06% |
| 10×10 matrix | 614 | 934 | 258 µs | 225 µs | 1.55% |
| Long row, 200 terms with fractions | 2970 | 4179 | 4.27 ms | 4.15 ms | 25.6% |

Parsing is a small part of every case; layout dominates once the tree is large.
The largest case is about 1.4 µs per node, so a formula would need roughly four
times its size (about 12 000 nodes, 16 KB of source) before a full reparse plus
layout crossed the frame budget on this machine.

## Decision

Full reparse plus full layout stays, and no incremental layout engine is built
now. The measured headroom is 25× for ordinary formulas and 4× for a formula
larger than any the corpus contains; the architecture's open decision asked for
a measurement before designing an incremental path, and this is that
measurement. The builder is not wired into the host yet, so the budget is
re-measured when a real editor's demo exists; the example stays in the tree for
that.

This leaves two conditions attached to the decision:

- if a formula approaches 12 000 nodes, or the measured pipeline exceeds half a
  frame on a host machine, the next step is caching the unchanged subtrees'
  boxes, not a faster full layout;
- the measurement covers parse and layout, not text shaping: `\text` runs go
  through the host's shaper and are outside this number.

## Verification

The example fails if any case produces a layout diagnostic, so the measured
corpus is also a compile-and-layout check. The numbers are not asserted in a
test because they are machine-dependent; the example is the reproducible
artifact.
