# LaTeX stack research

Staging area for an independent project: a self-contained LaTeX math stack
(parser with byte spans, TeX-style layout with node identity, editing geometry).

Not part of the mdBook; nothing here is linked from `docs/src/` and nothing in
the repository builds against it.

| File | What it is |
| --- | --- |
| `survey.md` | Synthesis across ten stacks: pipeline shapes, where spans die, editing architectures, font strategies, lessons |
| `architecture.md` | Target design: crates, model, layout, geometry, host contract, milestones, rejected alternatives |
| `raw/js.md` | KaTeX, MathJax, Temml, MathLive, MathQuill — source-cited notes |
| `raw/rust.md` | formulary, pulldown-latex, mathtex, ratex, Typst — source-cited notes |
| `raw/algorithm.md` | TeX spacing/style rules, OpenType MATH, stretch protocol, editor geometry, coverage |

Evidence rules used: a structural claim names a file and line from the source
revision in the report; anything unverified is marked. Measurements are only
reported where a stack published them or this work measured them.
