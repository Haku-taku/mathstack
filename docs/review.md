# Review report: model-to-LaTeX boundaries

This is the 6.4 audit: where a node tree the parser cannot produce is written as
LaTeX that means something else, and which of those states are reachable from
parsing or editing. The static checks (unsafe, panics, licenses, dependency
direction) are in 6.1/6.2 and are not repeated here.

## Method

A read-only review of `crates/math-syntax/src/writer.rs`,
`crates/math-core/src/node.rs` and the parser was run against `c635af9`. Every
candidate was reproduced with a temporary probe outside the repository; only
then was it fixed or recorded. A defect counts as high value when parsing or
editing can reach it; host-built model states are documented as known gaps.

Each fix below has a test that failed before it and passes after; the owning
design document carries a `## Corrections` paragraph.

## Fixed

| Finding | Cause | Fix | Test | Commit |
| --- | --- | --- | --- | --- |
| A declaration inserted mid-row colored or restyled the following siblings | the writer assumed a declaration was last in its row and never braced its body | `Context::ItemBeforeSibling`; a declaration braces its body when a sibling follows, and a script base is written as if one did | `a_declaration_inserted_mid_row_keeps_its_scope` | `91094dd` |
| A missing `\colorbox` body healed into literal text | the non-`Text` fallback serialized the `Error` node inside the text argument | only a `Text` body is written; anything else omits the body, so re-export keeps the error | `a_colorbox_without_a_body_writes_an_error_back` | `b1370f3` |
| `colorbox`/`fcolorbox`/`mathchoice`/`mathstrut` reported `Unknown` | the command shape table lagged the parser | registered as `Special`/`None` | `command_shapes_describe_what_the_parser_expects` | `0d64fa8` |
| A no-op `Cancel` wrote `\ {x}`; a no-op `Smash` became a two-way smash | flag combinations with no command were still serialized as one | the body alone is written for `(false, false)` | `a_no_op_cancel_or_smash_writes_its_body_alone` | `7382623` |
| A host-built `Arrow`/`UnderArrow` with `⇒` came back as `→` | the writer chose `\xrightarrow`/`\underrightarrow` for every non-left symbol | only ←/→ keep those commands; others use `\overset`/`\underset`, keeping symbol and labels | `a_foreign_arrow_keeps_its_symbol_and_labels` | `3d4781d` |
| A sized delimiter with a class outside Open/Close/Rel/Ord lost its height | no `(size, class)` command existed, so only the glyph was written | fall back to the size's plain `\big` family, losing only class spacing | `a_sized_delimiter_outside_the_command_classes_keeps_its_size` | `2a6c6a4` |
| KaTeX color arguments were rejected | names were matched case-sensitively and bare six-digit hex was not read | case-insensitive names, `aqua`/`fuchsia`/grey aliases, bare six-digit hex | `textcolor_and_color_write_back_their_own_forms` | `3ad1428` |

## Known gaps

- **Host-built no-op and foreign wrappers lose structure on write.** A
  `Cancel`/`Smash` with no flagged dimension, an `Arrow` with a symbol other
  than ←/→, and a sized delimiter with an unusual class have no exact LaTeX
  spelling. The writer preserves rendering, symbol and labels, but the reparse
  yields the plain body, an `OverUnder`, or a differently classed delimiter.
  The parser and editor never build these states; only a host calling
  `ModelBuilder`/`Model` does.
- **A color box body is a text slot.** `ColorBox` has a `NodeId` body for
  uniformity, but only `Text` has a spelling. A host that puts math there gets
  an empty body on write (the output then reports the missing argument) rather
  than a silent text conversion.
- **Color arguments are not byte-preserving.** A `\color` declaration keeps a
  spelling for `\color{…}`, but its text is reconstructed from the resolved
  color, so `\color{grey}` exports as `\color{gray}` and `\color{#f00}` as
  `\color{red}`. `\textcolor` has no spelling and always takes the canonical
  form. Revisit when the editor can enter a color argument.
- **The editor cannot type color or four-argument commands.**
  `colorbox`/`fcolorbox`/`mathchoice` have a `Special` shape, and
  `command_template` returns `None` for `Special`, so resolving the buffer
  reports `unknown command`. `\mathstrut` does insert. Supporting these needs a
  color-argument shape (or template) and, for colors, an editable color
  parameter; the same applies to the existing `color`/`textcolor`.
- **An unknown accent symbol writes `\hat`.** The writer has no diagnostic
  channel, and the parser never produces an accent outside the table; the
  fallback is documented at the writer arm.
- **`Model::set_root` does not free the old tree**, and `Model::node` on a
  freed id is a release-mode panic. `NodeId` is private and cannot be forged
  from input, so both need an internal stale reference; the editor's
  snapshot/delete tests do not cover freeing a node while a caret path still
  refers to it. No API change was made in this pass.
- **Top-level `\\`/`\newline`/`\par` stay unmodelled** by the decision recorded
  in `m5-long-tail.md`; a one-column table is not an equivalent model.
- **The color name table is closed at 27 names.** KaTeX passes any CSS keyword
  through to the renderer; resolving to RGB keeps layout independent of CSS, so
  other names are a diagnostic. Extending the table is additive.

## Not changed

The crabsnap-ng pipeline is untouched; `ui-integration.md` still records the
seam and the prerequisites for 7.2/7.3. This pass changed only mathstack.

## Verification

At `2a6c6a4`, `cargo fmt --all -- --check`, `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings` and
`cargo run -p math-diff -- <face> --gate` are green. Test targets: editor 31,
syntax 34, core 45 (plus geometry 6 and editing 3); the KaTeX probe baseline
is 336/987 with 28 environment names.
