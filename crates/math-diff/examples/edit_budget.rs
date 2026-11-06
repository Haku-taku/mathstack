//! Times the per-edit pipeline: parse the source, then lay it out.
//!
//! cargo run --release -p math-diff --example edit_budget -- <font.ttf>
//!
//! A keystroke that changes the source reparses the whole formula and lays it
//! out again, so this is the cost a host pays per edit. The report is a
//! measurement, not a gate: machine speed decides the absolute numbers.

use math_core::{Style, layout};
use math_font::Face;
use math_syntax::parse;
use std::error::Error;
use std::hint::black_box;
use std::time::Instant;

/// One 60 Hz frame, the budget a keystroke has to fit in.
const FRAME_MICROSECONDS: f64 = 16_700.0;

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: edit_budget <font.ttf>")?;
    let bytes = std::fs::read(path)?;
    let face = Face::parse(&bytes, 0)?;

    let matrix: String = {
        let mut source = String::from("\\begin{pmatrix}");
        for row in 0..10 {
            if row > 0 {
                source.push_str(" \\\\ ");
            }
            for column in 0..10 {
                if column > 0 {
                    source.push_str(" & ");
                }
                source.push_str(&format!("a_{{{row}{column}}}"));
            }
        }
        source.push_str("\\end{pmatrix}");
        source
    };
    let long_row = (0..200)
        .map(|index| format!("x_{index}^2+\\frac{{{index}}}{{n}}"))
        .collect::<Vec<_>>()
        .join("+");

    let cases: [(&str, &str); 4] = [
        ("inline formula", "\\frac{-b\\pm\\sqrt{b^2-4ac}}{2a}"),
        (
            "display sum",
            "\\sum_{i=1}^{n} i^2 = \\frac{n(n+1)(2n+1)}{6}",
        ),
        ("10x10 matrix", &matrix),
        ("long row", &long_row),
    ];

    const EDITS: usize = 1_000;
    println!(
        "{:<16} {:>6} {:>8} {:>12} {:>12} {:>10}",
        "case", "nodes", "bytes", "parse+layout", "layout only", "frame"
    );
    for (name, source) in cases {
        let document = parse(source);
        let nodes = document.model.len();
        let (_, diagnostics) = layout(&face, &document.model, document.root(), 16.0, Style::TEXT);
        if !diagnostics.is_empty() {
            return Err(format!("{name}: {:?}", diagnostics).into());
        }

        let start = Instant::now();
        for _ in 0..EDITS {
            let document = parse(black_box(source));
            let (layout, _) = layout(&face, &document.model, document.root(), 16.0, Style::TEXT);
            black_box(&layout);
        }
        let edit = start.elapsed().as_secs_f64() * 1e6 / EDITS as f64;

        let start = Instant::now();
        for _ in 0..EDITS {
            let (layout, _) = layout(&face, &document.model, document.root(), 16.0, Style::TEXT);
            black_box(&layout);
        }
        let relayout = start.elapsed().as_secs_f64() * 1e6 / EDITS as f64;

        println!(
            "{name:<16} {nodes:>6} {:>8} {:>10.1} us {:>10.1} us {:>9.2}%",
            source.len(),
            edit,
            relayout,
            100.0 * edit / FRAME_MICROSECONDS
        );
    }
    Ok(())
}
