//! Paints a `Layout` as SVG.
//!
//! Glyph paths are outline geometry, not text, so the output is identical on
//! every machine and needs no font at view time. That is also what makes this
//! the CI fidelity path: rasterise the SVG and compare pixels.

use math_core::{Color, GlyphOutline, Item, Layout, PathSink};
use std::fmt::Write as _;

pub struct Options {
    pub padding: f32,
}

impl Default for Options {
    fn default() -> Self {
        Options { padding: 8.0 }
    }
}

pub fn to_svg(layout: &Layout, font: &dyn GlyphOutline, options: &Options) -> String {
    let padding = options.padding;
    let width = layout.width + padding * 2.0;
    let height = layout.ascent + layout.descent + padding * 2.0;
    let mut svg = String::new();
    let _ = write!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" \
         viewBox=\"0 0 {} {}\"><g fill=\"#000000\">",
        num(width),
        num(height),
        num(width),
        num(height)
    );
    // Backgrounds are their own primitive, so they are painted before every
    // glyph, rule and line regardless of where they sit in the list.
    for item in &layout.items {
        if let Item::Background(background) = item {
            let _ = write!(
                svg,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/>",
                num(background.x + padding),
                num(background.y + padding),
                num(background.width),
                num(background.height),
                paint(Some(background.color), "fill"),
            );
        }
    }
    for item in &layout.items {
        match item {
            Item::Background(_) => {}
            Item::Glyph(glyph) => {
                let mut path = PathWriter::default();
                if !font.outline(glyph.glyph, &mut path) {
                    continue;
                }
                let _ = write!(
                    svg,
                    "<path transform=\"translate({} {}) scale({} {})\" d=\"{}\"{paint}/>",
                    num(glyph.x + padding),
                    num(glyph.baseline + padding),
                    num(glyph.size),
                    num(-glyph.size),
                    path.data,
                    paint = paint(glyph.color, "fill"),
                );
            }
            Item::Rule(rule) => {
                let _ = write!(
                    svg,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{paint}/>",
                    num(rule.x + padding),
                    num(rule.y + padding),
                    num(rule.width),
                    num(rule.height),
                    paint = paint(rule.color, "fill"),
                );
            }
            Item::Line(line) => {
                let _ = write!(
                    svg,
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" \
                     stroke=\"{}\"{} stroke-width=\"{}\"/>",
                    num(line.x1 + padding),
                    num(line.y1 + padding),
                    num(line.x2 + padding),
                    num(line.y2 + padding),
                    line.color.map_or("#000000".to_owned(), hex),
                    opacity(line.color, "stroke"),
                    num(line.thickness)
                );
            }
        }
    }
    svg.push_str("</g></svg>");
    svg
}

/// A `fill`/`stroke` override, or nothing so the group's default shows through.
fn paint(color: Option<Color>, attribute: &str) -> String {
    let Some(color) = color else {
        return String::new();
    };
    format!(
        " {attribute}=\"{}\"{}",
        hex(color),
        opacity(Some(color), attribute)
    )
}

fn opacity(color: Option<Color>, attribute: &str) -> String {
    match color {
        Some(color) if !color.is_opaque() => format!(
            " {attribute}-opacity=\"{}\"",
            num(f32::from(color.alpha) / 255.0)
        ),
        _ => String::new(),
    }
}

fn hex(color: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.red, color.green, color.blue)
}

#[derive(Default)]
struct PathWriter {
    data: String,
}

impl PathWriter {
    fn separator(&mut self) {
        if !self.data.is_empty() {
            self.data.push(' ');
        }
    }
}

impl PathSink for PathWriter {
    fn move_to(&mut self, x: f32, y: f32) {
        self.separator();
        let _ = write!(self.data, "M{} {}", num(x), num(y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.separator();
        let _ = write!(self.data, "L{} {}", num(x), num(y));
    }

    fn quad_to(&mut self, control_x: f32, control_y: f32, x: f32, y: f32) {
        self.separator();
        let _ = write!(
            self.data,
            "Q{} {} {} {}",
            num(control_x),
            num(control_y),
            num(x),
            num(y)
        );
    }

    fn curve_to(
        &mut self,
        control1_x: f32,
        control1_y: f32,
        control2_x: f32,
        control2_y: f32,
        x: f32,
        y: f32,
    ) {
        self.separator();
        let _ = write!(
            self.data,
            "C{} {} {} {} {} {}",
            num(control1_x),
            num(control1_y),
            num(control2_x),
            num(control2_y),
            num(x),
            num(y)
        );
    }

    fn close(&mut self) {
        self.data.push('Z');
    }
}

/// Three decimals keep output stable without visible quantisation.
fn num(value: f32) -> String {
    let mut text = format!("{value:.3}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        text = "0".to_owned()
    }
    text
}
