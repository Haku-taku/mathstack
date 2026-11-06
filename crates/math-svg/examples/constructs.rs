//! Renders a LaTeX formula through the parser, layout and SVG painter.
//!
//! cargo run -p math-svg --example constructs -- <font.ttf> [latex] [display]

use math_core::Style;
use math_font::Face;
use math_svg::{Options, to_svg};
use std::{env, error::Error, fs};

const DEFAULT: &str = "x^2+a_i+\\left(\\frac{a+b}{2}\\right)=\\sqrt[3]{c+d}+\\sum_{i=1}^{n}";

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args().skip(1);
    let font_path = arguments
        .next()
        .ok_or("usage: constructs <font.ttf> [latex]")?;
    let formula = arguments.next().unwrap_or_else(|| DEFAULT.to_owned());
    let display = arguments.next().as_deref() == Some("display");
    let bytes = fs::read(&font_path)?;
    let face = Face::parse(&bytes, 0)?;

    let document = math_syntax::parse(&formula);
    for diagnostic in &document.diagnostics {
        eprintln!(
            "{}..{}: {}",
            diagnostic.span.start, diagnostic.span.end, diagnostic.message
        );
    }

    let style = if display { Style::DISPLAY } else { Style::TEXT };
    let (layout, diagnostics) =
        math_core::layout(&face, &document.model, document.root(), 48.0, style);
    for diagnostic in &diagnostics {
        eprintln!("{}", diagnostic.message);
    }
    println!("{}", to_svg(&layout, &face, &Options::default()));
    Ok(())
}
