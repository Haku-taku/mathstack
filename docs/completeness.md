# Completeness gates

The research note `docs/research/latex-stack/testing.md` (in the crabsnap
checkout) rejects a single "LaTeX conformance" suite and proposes five layered
gates. This page records what of that harness exists here, with the numbers
from a run on 2026-11-07, and what remains a gap.

## The construct table

The completeness claim is a table, not a percentage. Prism a command list from
KaTeX's MIT-licensed `docs/supported.md` and probe each entry:

- `crates/math-syntax/tests/data/katex_supported.txt` — 954 control sequences;
- `crates/math-syntax/tests/data/katex_environments.txt` — 33 environment names;
- `crates/math-syntax/tests/data/katex_accepted.txt` — the 316 entries the
  probes accept today, frozen as a regression baseline.

A probe is one of a few generic uses (`\cmd`, `\cmd{a}`, `\cmd{a}{b}`,
`\cmd[1]{a}`, `\cmd*{a}`, `{a \cmd b}`, `\cmd{1em}`, three arguments) plus a
few grammar-specific ones (`\left`, `\middle`, `\genfrac`, `\def`,
`\newcommand`). Acceptance means one probe parses without a diagnostic or an
`Error` node. This is parse coverage: it cannot say whether the command renders
correctly, and a command with unusual arity can be a false negative.

## Gates

| Gate | State | How |
| --- | --- | --- |
| L1 parse | Partial | 77/77 curated constructs parse (`conformance.rs`); KaTeX list 316/987 through the probe set; `katex_supported_list_does_not_regress` fails if a baselined entry stops parsing; arbitrary text never panics |
| L2 structure | Partial | Every non-whitespace byte of the curated corpus is inside a node span or a diagnostic (`every_corpus_byte_is_covered_by_a_span_or_diagnostic`); the writer round-trips the supported subset structurally |
| L3 geometry | Partial | `math-diff --gate` checks the 16-formula differential against per-case documented allowances (±0.01 elsewhere) and fails on any parse or layout diagnostic; the rule-choice cases exceed the note's blanket 1% width threshold |
| L4 visual | Partial | The native host in `math-editor-demo` paints the display list through `resvg` and draws caret and selection; there is no golden-image or automated screenshot gate |
| L5 robustness | Present | `arbitrary_text_never_panics` (3 000 generated sources) and `every_prefix_of_the_corpus_parses`; macro, depth and node budgets have their own tests |

The L1 number is low because the KaTeX list includes colour, HTML, sizing,
hundreds of relation/arrow symbols and environments this engine does not claim.
That is the point of publishing it: the supported subset is explicit instead of
being implied by a curated checklist.

## Known gaps

- **L3 is not the 1% blanket threshold.** Five cases (both scripts, fraction
  row, sum with limits, integral side scripts, nested script) diverge by more
  than 1% because the two engines place scripts by different rules; the gate
  encodes those documented amounts instead of pretending they pass. A change
  that moves any of them still fails, which is the regression property.

- **Macro definitions are not in the model.** `\def`/`\newcommand` bodies are
  consumed by the expander and leave no node, so L2 skips formula sources that
  define macros, and the writer does not reproduce a definition. The call sites
  are marked (`Origin::Macro`), which is enough for read-only editing but not
  for byte coverage.
- **No external corpora yet.** MathJax's JSON cases and im2latex-100k are not
  wired in; the note requires them to be fetched, never committed unless
  redistribution is allowed.
- **No visual gate.** L4 now has a native host (`math-editor-demo`), but its
  screenshots are manual; no golden images are compared in CI.
- **Probe heuristics undercount.** Commands such as `\color` or `\rule` with a
  required third argument are not attempted if no template matches their
  grammar; adding a probe is a deliberate widening of the claim.

## Running

```sh
cargo test -p math-syntax --test completeness -- --nocapture
```

Regenerating the baseline is an explicit act, not a side effect:

```sh
KATEX_BASELINE="$PWD/crates/math-syntax/tests/data/katex_accepted.txt" \
  cargo test -p math-syntax --test completeness katex_supported_list
```

The differential gate needs a MATH font:

```sh
cargo run -p math-diff -- /path/to/MathFont.ttf --gate
```

## See also

- [`architecture.md`](architecture.md) — the validation section this expands.
- [`m2-parser.md`](m2-parser.md) — the parser and writer the L1/L2 gates exercise.
- [`host-editor.md`](host-editor.md) — the native renderer and its manual check.
- [`m1-differential.md`](m1-differential.md) — the recorded L3 deltas.
