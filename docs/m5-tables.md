# M5, part 1: tables and environments

M5 spans four notes: tables and aligned environments, the long tail of
constructs the conformance checklist deferred, the per-edit budget, and the
editing operations. This note covers the first: a table node shared by
matrices, `cases` and later `aligned`, and the environment parsing and writing
that feed it.

## Model

```rust
Table { rows: Vec<NodeId>, columns: usize, alignment: Vec<Alignment> }
TableRow { cells: Vec<NodeId> }
```

A cell is an ordinary row node, so a cell holds any formula and the editing
model already addresses it. The table owns column count and alignment; a row
owns only its cells. `Slot::Row` and `Slot::Cell` name these children, which is
what caret paths and hit testing use. Rows are padded with empty rows to the
widest row's column count at parse time, so layout never has to handle a ragged
grid.

`alignment` is `Left`, `Center` (the default when an entry is missing) or
`Right`. It is a per-table list because the table is also the place where
`aligned` will set its alternating right/left columns.

## Layout

`layout_table` measures every cell once, takes each column's width as the
maximum over its cells, and lays rows out top to bottom:

- column gap and row gap are 0.5em;
- a row's baseline advances by the previous row's descent plus the row gap plus
  the row's ascent, so rows with tall cells do not overlap;
- each cell is shifted inside its column by the alignment (left, half the slack,
  or all of it);
- table ascent and descent are the extrema over rows, including the baseline
  offsets, so the table box encloses every row.

Offsets in the box tree are y-down: a later row's baseline offset is positive.
A row node is placed with that offset and its cells at the row's own baseline.

`layout_table_row` lays the cells of a standalone row (a cell outside a table,
which the parser can still produce through recovery) left to right with the
same 0.5em gap.

## Environments

The parser reads `\begin{name}` … `\end{name}` for `matrix`, `pmatrix`,
`bmatrix`, `Bmatrix`, `vmatrix`, `Vmatrix`, `cases`, `aligned` and `align`.
Row parsing gains a `Cell` stop: `&` and `\\` end a cell, `\end` ends the
environment. A trailing `\\` before `\end` contributes no empty row. The
delimited families wrap the table in `Delimited { left, body: Row([table]),
right }` with the environment's fences; `matrix` returns the bare table.

`cases` and `aligned` are the environments with semantic alignment: `cases`
sets every column `Left`, while `aligned` alternates `Right`, `Left`, `Right`
across its columns. `cases` is written back as `\begin{cases}…\end{cases}` by
recognising its shape in the `Delimited` writer arm, not as
`\left\{\begin{matrix}…`, so the alignment survives a round trip. `aligned` is
written by recognising the alternating column list in the `Table` arm.

The writer emits `\begin{matrix}` for a table, ` & ` between cells and ` \\\\ `
between rows. A table that was read from any environment name other than
`aligned` is written as a matrix; the fences, where they existed, are written
separately.

## Geometry

Hit testing descends a table like any other multi-child node, with one
difference: `Slot::Row` compares the point's y against each row's vertical
midpoint, while `Slot::Cell` and every other slot compare x. The path therefore
reads table → row → cell → position inside the cell, and every step resolves
back to the same caret.

## Verification

- `matrix_columns_share_their_widest_cell` — a 2×2 table whose column widths
  come from the wider cell lays out at the summed column widths plus one gap,
  and its second row's baseline is the first row's descent plus the gap plus the
  second row's ascent.
- `table_alignment_places_narrow_cells_in_their_column` — a left-aligned table
  puts a narrow cell at its column's left edge, while the default centring moves
  it half the slack; both are checked against the glyph x positions.
- `hit_test_descends_a_table_row_then_its_cell` — a point inside the second
  row's second cell yields `Row(1)`, then `Cell(1)`, then a boundary inside that
  cell.
- `matrix_environments_build_rows_and_cells` — the parser produces the expected
  rows and cells for `pmatrix`, pads a short row and gives `cases` its left
  alignment and `{` fence.
- The conformance checklist gained `matrix`, `pmatrix`, `bmatrix`, `cases` and
  `aligned`; the wider KaTeX-list gate is in
  [`completeness.md`](completeness.md).

## Corrections

- **Table rows stacked upwards.** The table layout placed row *i* at
  `-baseline`, but box-tree offsets are y-down; the second row was drawn above
  the first. Extents did not notice, because ascent and descent were computed
  from the same sign, and the published glyph positions of the differential
  corpus contain no table. The hit-test test caught it: a point in the second
  row selected row 0. The offset is now `+baseline`.
- **`cases` lost its alignment in the writer.** Writing the parsed `cases` as
  `\left\{\begin{matrix}…\right.` reparsed into a centred table, so the
  round-trip test failed on the dump. The `Delimited` writer arm now recognises
  the cases shape (left fence `{`, no right fence, a single table body whose
  columns are all left-aligned) and writes the environment name instead of the
  generic form.
