# mathstack

A self-contained LaTeX math stack: `math-core` (model, layout, geometry),
`math-syntax` (lexer, parser, writer), `math-font`, `math-svg`, `math-diff`,
`math-editor`, `math-editor-demo`.

Before changing a subsystem, read `docs/architecture.md` and the owning design
document (including its `## Corrections`). Documentation lives in `docs/`.

## Build and verify

Run from the repository root:

```sh
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --tests --examples
cargo run -p math-diff -- /usr/share/fonts/wps-office/DejaVuMathTeXGyre.ttf --gate
```

Use `--release` for look/performance checks; debug only for fast compile loops.
Graphical checks run inside a nested session:

```sh
/home/ollie/git/crabsnap-ng/scripts/nested-session.sh start
eval "$(/home/ollie/git/crabsnap-ng/scripts/nested-session.sh env)"
./target/release/math-editor-demo
/home/ollie/git/crabsnap-ng/scripts/nested-session.sh shot /tmp/shot.png
/home/ollie/git/crabsnap-ng/scripts/nested-session.sh stop
```

## Rules

- Fix root causes; do not patch symptoms. Reuse existing facilities before adding
  new ones.
- Zero warnings is the bar. No `unwrap()`/`expect()` outside tests/examples.
- Comments explain why, never what; at most two lines.
- Do not add new `mod.rs` files; name new library roots in `Cargo.toml`.
- No compile-time `cfg` to choose between backends.
- Unit tests only for major branches (fallback chains), complex algorithms, and
  regressions of major defects. Ask when unsure; do not pile up snapshots.
- Keep the arena invariant: `ModelBuilder::build` marks only nodes reachable from
  the root alive. Editor exports normalize spans through the writer.
- Editing layout (`layout_with_geometry`) reserves empty slots and includes a
  pending command; static layout and the writer do not.
- Update the owning design document and its `## Corrections` when fixing a
  mistake. State how each claim was checked.
- Do not commit worktree changes without the main session's decision; report
  changed files and verification to the session that assigned the task.
