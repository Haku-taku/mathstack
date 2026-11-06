# M3: macros, font variants, accents and text

This milestone widens the accepted language without changing the layout
contract: a macro expands to tokens, a font command selects a variant stored on
the atom, an accent is a model node, and `\text` is a raw-slice node. The
second half adds the constructs that are grammar rather than new layout —
spacing commands, fixed and middle delimiters, named operators, bars, infix
fractions, style overrides and the phantom family — and closes with a curated
KaTeX conformance checklist.

## Macro expansion

`math-syntax/src/macros.rs` runs between the lexer and the parser. Tokens, not
strings, are substituted, so spans survive.

```
\def\name#1#2{body}            \newcommand{\name}[2]{body}
\renewcommand{\name}[2]{body}  \newcommand\name[2]{body}
```

- Arguments are undelimited: a braced group (one level of braces removed) or a
  single token. A missing argument becomes empty and reports one diagnostic.
- A parameter is `#` followed by a digit; parameters must be numbered
  consecutively in `\def`. `##` is not special (literal `#`).
- A body token other than a parameter keeps the **call site's span**; an
  argument's tokens keep their own spans. Every expanded byte is therefore
  covered by the call that produced it.
- Definitions are formula-scoped. Braces do not create macro scopes, and there
  is no `\edef`, delimited parameter text, default arguments or catcode work.
- Budgets: 1 000 expansions and 100 000 expanded tokens. Exceeding either
  reports one diagnostic and leaves an `Error` node at the call; expansion never
  recurses on the Rust stack, so a self-recursive macro terminates.
- `\newcommand` on a defined name and `\renewcommand` on an undefined one report
  a diagnostic; `\newcommand` keeps the old definition.

`Origin` (definition and call site, for editing expanded content) is deferred
to M4. Until then, expanded nodes share the call span and the definition is not
retained.

## Font variants

`Atom { symbol, class, variant }`; `variant` is one of `Normal`, `Upright`,
`Bold`, `Italic`, `BoldItalic`, `Script`, `Fraktur`, `DoubleStruck`,
`SansSerif`, `Monospace`. Commands: `\mathrm`, `\mathbf`, `\mathit`,
`\mathsf`, `\mathtt`, `\mathcal`, `\mathbb`, `\mathfrak`, `\boldsymbol`,
`\mathnormal`.

- The parser applies a command's variant to everything it parses inside the
  argument; nested commands override it.
- Layout resolves in this order: the variant's Unicode codepoint if the face has
  it, the base symbol for non-`Normal` variants, then the Mathematical Italic
  form for ordinary letters. `Normal` and `Upright` have no variant codepoint:
  `Normal` italicises Latin letters, `Upright` does not.
- Script, fraktur and double-struck have unassigned slots where the letters live
  in Letterlike Symbols; `variant_char` maps those explicitly (for example
  `\mathbb{R}` is U+211D, `\mathcal{B}` U+212C, `\mathcal{e}` U+212F).
- Greek and symbols with no styled codepoint fall back to the base glyph rather
  than inventing one.
- The writer wraps each non-`Normal` atom in its command. Because the parser
  unwraps a one-child argument, `\mathbb{R}` round-trips exactly; a group's
  atoms each get their own wrapper, which reparses to the same variants.

## Accents

`\hat`, `\widehat`, `\tilde`, `\widetilde`, `\bar`, `\vec`, `\dot`, `\ddot`,
`\acute`, `\grave`, `\breve`, `\check`, `\mathring` become
`Accent { base, symbol }`. The mark's codepoint follows KaTeX (the spacing
marks, e.g. U+02C6, U+02DC, U+00AF, U+20D7).

Placement follows MathML Core and formulary:

- the accent keeps the base's style size (no script reduction);
- its baseline sits at `AccentBaseHeight` above the baseline, and rises by the
  amount a taller base exceeds that constant;
- horizontally, the base's MATH top-accent attachment aligns with the mark's
  own attachment; boxes without a lone glyph (or faces without the table) use
  their centres.

`\widehat`/`\widetilde` currently draw the fixed mark. Stretching them needs
horizontal MATH variants, which `math-font` does not read yet; `\bar{x+y}`
therefore shows TeX's short bar over a wide base rather than an overline.

## Text

`\text{…}` stores the raw source between its braces as `Text(String)`: spaces
survive, nested groups survive, and commands inside are literal for this
milestone. A `\text` written inside a macro body has no recoverable source (its
tokens carry the call span), so it reports a diagnostic and becomes an `Error`
node; text passed as a macro argument is not preserved either. Layout now shapes the text through `TextShaper`. The core fallback,
`FontShaper`, draws upright glyphs one per character with the face's advances
and ink, so spaces take width but paint nothing; a missing glyph reports a
diagnostic with the node's span. A host that wants kerning, ligatures or font
fallback installs its own shaper. `\operatorname` takes the same literal text
(see below), so what it stores is raw characters, not a math row.

## Spacing commands

`\,`, `\:`, `\;`, `\!`, `\enspace`, `\quad`, `\qquad` and `\ ` become
`Space(SpaceKind)` with the TeX widths (thin 3/18 em, medium 4/18, thick 5/18,
negative thin, 0.5/1/2 em, normal 1/3 em). A space node has no atom class: the
row skips it when it pairs neighbours for the 8×8 tables, so `a\,+b` still
reads Ord–Bin–Ord and only gains a thin gap before the operator. That is TeX's
own scan, which looks past glue rather than treating it as an ordinary atom.
Spaces carry no ink and are dropped by the writer's `push_command` spacing rules
like any other command.

## Fixed and middle delimiters

`\big`, `\Big`, `\bigg`, `\Bigg` and the `l`/`r`/`m` suffixes become
`SizedDelimiter { symbol, size, class }` with fixed targets of 1.2, 1.8, 2.4 and
3.0 em. The suffix only sets the spacing class (`\bigl(` is Open, `\bigm|` is
Rel); the height is independent of it. The glyph comes from the same
stretch-variant protocol as `\left…\right`, so a delimiter with no variant at
that size keeps its largest drawing rather than scaling outlines.

`\middle` is legal only inside `\left…\right`. The group's final height is known
only after its body is measured, so layout probes the body once without the
middles, takes `2·max(ascent − axis, descent + axis)`, and lays the body out
again with every middle at that height. The fences then size to the second
layout. A `\middle` outside a group is a `Middle` node the parser reports.

## Named operators

`\sin`, `\cos`, `\tan`, `\cot`, `\sec`, `\csc`, `\arcsin`, `\arccos`,
`\arctan`, `\sinh`, `\cosh`, `\tanh`, `\coth`, `\log`, `\ln`, `\lg` and `\exp`
become `Function { name, limits: false }`; `\lim`, `\limsup`, `\liminf`,
`\max`, `\min`, `\sup`, `\inf`, `\det`, `\gcd` and `\Pr` set `limits: true`.
`\operatorname{…}` takes its literal text as the name and `\operatorname*`
forces limits. A limits function puts its scripts above and below in display
style and beside it otherwise; a plain function always uses side scripts. The
name is drawn upright in the surrounding size, with the same atom spacing as an
operator.

## Bars and infix fractions

`\overline` and `\underline` become `Overline`/`Underline`: a rule as wide as
the base, placed using the MATH Overbar/Underbar constants (vertical gap, rule
thickness, extra ascender or descender) so it clears the body's ink.

The infix forms `\over`, `\atop`, `\choose`, `\brace` and `\brack` split the
enclosing group into a fraction; without braces they split the whole row.
`\over` draws the rule, `\atop` omits it, and the remaining three wrap the
barless fraction in `( )`, `{ }` or `[ ]`. The parser rewrites the group into
`Fraction` (plus `Delimited` where needed) and never nests an extra row, which
is what keeps `a \atop b` round-tripping without braces.

`\frac` inherits the surrounding style; `\dfrac` and `\tfrac` store
`forced: Some(true/false)` and layout substitutes display or text style before
choosing gaps. The flag lives in the model rather than being baked into the
children, so relayout under another style still follows the command.

## Negation and phantoms

`\not` negates the next atom through a table (`=`→`≠`, `∈`→`∉`, `<`→`≮`,
`⊂`→`⊄`, `∼`→`≁`, `∣`→`∤`, …). A target with no negated codepoint, or a target
that is not an atom, reports a diagnostic and keeps the original node.

`\phantom`, `\hphantom` and `\vphantom` become `Phantom { body, kind }`: the
body is laid out and then its dimensions are handed over while its items are
dropped (full, width only, height only). `\smash[t]`/`\smash[b]` do the reverse
— they keep the ink and drop one extent. `\boxed` adds a framed rule with 0.3 em
padding and a 0.04 em stroke. `\overset` and `\underset` become `OverUnder`,
placing the annotation above or below the base with the stack constants.

## Conformance checklist

`crates/math-syntax/tests/conformance.rs` holds two lists. `CORE` has 60
constructs this milestone claims — rows, symbols, scripts, all fraction forms,
radicals, delimiters, large operators, functions, accents, text, bars, spacing,
variants, macros, `\not` and the phantom family — and each must parse without a
diagnostic. `GAPS` has 12 constructs knowingly deferred to M5, and each must
still fail:

| Deferred construct | Why it waits |
| --- | --- |
| `\stackrel`, `\substack` | stacks and multi-line limits |
| `\cfrac`, `\genfrac` | continued fractions with their own style rules |
| `\xrightarrow`, `\xleftarrow` | horizontally stretchy arrows |
| `\underbrace`, `\overbrace` | stretchy braces plus annotation placement |
| `\pmod`, `\bmod` | modular-notation spacing as a unit |
| `\cancel` | diagonal strike needs a new display item |
| `\hspace` | explicit lengths are a host unit decision |

The ratio was 60/72 = 83%, above the milestone's 80% bar. The two-list shape was
deliberate: a deferred construct that started parsing cleanly failed
`tracked_gaps_are_still_gaps` and had to be promoted to `CORE`, so the
percentage could not creep upward by quietly re-labelling work. All twelve were
promoted in M5; the list is empty and that test is retired, while the wider
KaTeX-list probe gate in [`completeness.md`](completeness.md) covers drift
beyond the curated list. Environments, colour, tags,
`\raisebox`, `\mathllap` and style commands are outside this checklist and
belong to M5's long tail.

## Verification

- `macros_expand_to_the_model` — `\def` and `\newcommand` with parameters,
  nested expansion, and the self-recursion budget.
- `variants_resolve_to_their_unicode_block` — the Letterlike-Symbol holes and
  the layout resolution order.
- `accents_center_on_the_math_attachment_and_clear_the_base` — the attachment
  offset, the AccentBaseHeight lift and the box extents.
- `text_keeps_spaces_and_their_advances` — raw text layout.
- `spacing_commands_are_glue_not_atoms` — a space node is skipped by the class
  scan and keeps the neighbouring Bin–Ord glue.
- `sized_delimiters_use_their_fixed_height` — the four fixed targets.
- `middle_delimiters_grow_with_the_enclosing_group` — the probe-and-relayout
  height.
- `bars_span_the_base_and_clear_it_by_the_math_gaps` — overline/underline rules
  and their MATH gaps.
- `over_and_under_scripts_clear_the_base` — `\overset`/`\underset` stack
  constants.
- `forced_fraction_styles_change_the_child_size` — `\dfrac` under text style
  uses display-size children.
- `phantoms_smash_and_boxes_trade_space_for_ink` — the phantom kinds, `\smash`
  directions and the `\boxed` frame.
- `writer_round_trips_the_grammar` — accents, variants, `\text`,
  macro-expanded sources, bars, spacing, the `\big` family, `\middle`,
  functions, infix fractions and the phantom family; every source except the
  deliberate unknown command must also parse cleanly.
- `core_constructs_parse_cleanly` and `checklist_coverage_meets_the_milestone`
  — the conformance list above; the wider gate is
  `katex_supported_list_does_not_regress` in `completeness.rs`.
- The M1 differential still parses its corpus and prints the same extents.

## Corrections

- **A construct in the model and writer is not yet in the language.**
  `\overline` and `\underline` had a `NodeKind`, layout support and writer
  output, and the writer's round-trip list already contained both — but
  `parse_command` had no arm for them, so each parsed as an unknown-command
  `Error`. The round trip still passed: an error node prints back as the text
  that failed to parse, and the test compared two parses of that text. The
  conformance checklist caught it. The round-trip test now also asserts that
  every source except its deliberate unknown command parses without
  diagnostics, and a missing parser arm fails for the reason it should.
