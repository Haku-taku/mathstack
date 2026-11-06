# Third-party code and data

This project adapts code and data from the projects below. An adapted file keeps
its upstream copyright and license header; this page is the full accounting.
Nothing here is vendored yet unless the section says so.

## KaTeX 0.16.22

In use. `crates/math-core/src/class.rs` carries the inter-atom spacing tables
from `src/spacingData.ts` (`spacings`, `tightSpacings`); the binary-operator
cancellation in `crates/math-core/src/layout.rs` follows `src/buildHTML.ts`.
The script shifts implement Appendix G Rule 18 as written in
`src/functions/supsub.js`, and the style transitions are those of `src/Style.js`.
`crates/math-syntax/src/symbols.rs` takes its command-to-codepoint choices,
atom classes and accent marks from `src/symbols.js` and
`src/functions/accent.js`. These are data and rules, not KaTeX source code.

MIT License, Copyright (c) 2013-2020 Khan Academy and other contributors.

## formulary 0.3.0

In use, adapted. `math-font`'s vertical stretch and assembly follow
`src/font.rs:280-359`, and its `ssty` subtable reading follows
`src/font.rs:365-410`; `math-core/src/layout.rs` takes the fraction, bar-less
stack, radical, limits, stretchy-delimiter and accent geometry from
`src/layout.rs:693-787,796-940,947-986,1280-1337,1676-1762`, rewritten for this
box type rather than wrapped.

MIT OR Apache-2.0; this project takes it under the MIT option,
Copyright (c) 2026 Jeffrey T. Peckham (Ophymx).

## pulldown-latex 0.8.0

Not used. M2 was planned as a fork, but the crate's event stream is shaped
toward MathML and carries no spans: deriving this model tree from it would have
replaced its output layer while keeping only its tables. `math-syntax` is
written against the same grammar instead, with pulldown-latex read as a
behaviour reference. No code or tables from it are included.

MIT License, Copyright 2024 Charles Edward Gagnon.

## Typst 0.15.1

Reference only. MATH constant fallbacks and the three-step stretch protocol are
ported as algorithms from `typst-library/src/text/font/metrics.rs` and
`typst-layout/src/math/glyph.rs`; no Typst code is included. If code is copied
later, its Apache-2.0 headers and NOTICE are retained.

Apache-2.0, Copyright the Typst contributors.

## Reference-only projects

The following informed the design but contribute no code: ratex (MIT),
MathLive (MIT), MathJax (Apache-2.0), Temml (MIT), mathtex (MIT OR Apache-2.0).
MathQuill is MPL-2.0, whose file-level copyleft this project does not adopt;
only its behaviour is referenced.

## MIT License text

Applies to the MIT-licensed sources above.

```text
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
