//! Renders a row of symbols to SVG with a math font.
//!
//! cargo run -p math-svg --example render -- <font.ttf> "a+b=c"

use math_core::{Model, Style, layout};
use math_font::Face;
use math_svg::{Options, to_svg};
use std::{env, error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args().skip(1);
    let font_path = arguments.next().ok_or("usage: render <font.ttf> [text]")?;
    let text = arguments.next().unwrap_or_else(|| "a+b=c".to_owned());
    let bytes = fs::read(&font_path)?;
    let face = Face::parse(&bytes, 0)?;
    let (model, root) = Model::build(|model| {
        let atoms: Vec<_> = text
            .chars()
            .filter(|symbol| !symbol.is_whitespace())
            .map(|symbol| model.symbol(symbol))
            .collect();
        model.row(atoms)
    });
    let (layout, diagnostics) = layout(&face, &model, root, 48.0, Style::TEXT);
    for diagnostic in &diagnostics {
        eprintln!("{}", diagnostic.message);
    }
    println!("{}", to_svg(&layout, &face, &Options::default()));
    Ok(())
}
