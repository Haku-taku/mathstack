//! Color names and hex values for `\color` and `\textcolor`.
//!
//! The names are the common CSS/dvips set KaTeX receives as CSS keywords; a
//! name outside the table is a diagnostic rather than a silent fallback.

use math_core::Color;

/// The names `\color` accepts, with the first one winning when writing back.
/// Matching is case-insensitive; aliases follow their canonical spelling.
const NAMED: [(&str, Color); 27] = [
    ("black", Color::rgb(0, 0, 0)),
    ("white", Color::rgb(255, 255, 255)),
    ("gray", Color::rgb(128, 128, 128)),
    ("red", Color::rgb(255, 0, 0)),
    ("green", Color::rgb(0, 128, 0)),
    ("blue", Color::rgb(0, 0, 255)),
    ("cyan", Color::rgb(0, 255, 255)),
    ("magenta", Color::rgb(255, 0, 255)),
    ("yellow", Color::rgb(255, 255, 0)),
    ("orange", Color::rgb(255, 165, 0)),
    ("purple", Color::rgb(128, 0, 128)),
    ("brown", Color::rgb(165, 42, 42)),
    ("lime", Color::rgb(0, 255, 0)),
    ("olive", Color::rgb(128, 128, 0)),
    ("pink", Color::rgb(255, 192, 203)),
    ("teal", Color::rgb(0, 128, 128)),
    ("violet", Color::rgb(238, 130, 238)),
    ("navy", Color::rgb(0, 0, 128)),
    ("maroon", Color::rgb(128, 0, 0)),
    ("silver", Color::rgb(192, 192, 192)),
    ("darkgray", Color::rgb(169, 169, 169)),
    ("lightgray", Color::rgb(211, 211, 211)),
    ("grey", Color::rgb(128, 128, 128)),
    ("aqua", Color::rgb(0, 255, 255)),
    ("fuchsia", Color::rgb(255, 0, 255)),
    ("darkgrey", Color::rgb(169, 169, 169)),
    ("lightgrey", Color::rgb(211, 211, 211)),
];

/// Parses a color argument: a case-insensitive table name, a bare six-digit
/// hex (`xcolor`'s HTML model, as KaTeX accepts), or `#`-prefixed hex
/// (`#RGB`, `#RGBA`, `#RRGGBB`, `#RRGGBBAA`; the alpha forms extend KaTeX).
pub(crate) fn parse(spec: &str) -> Result<Color, String> {
    let spec = spec.trim();
    if let Some(hex) = spec.strip_prefix('#') {
        return parse_hex(hex).ok_or_else(|| format!("invalid color `#{hex}`"));
    }
    if spec.len() == 6 && spec.chars().all(|symbol| symbol.is_ascii_hexdigit()) {
        return parse_hex(spec).ok_or_else(|| format!("invalid color `{spec}`"));
    }
    let name = spec.to_ascii_lowercase();
    NAMED
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, color)| *color)
        .ok_or_else(|| format!("unknown color `{spec}`"))
}

fn parse_hex(hex: &str) -> Option<Color> {
    let digits: Vec<u8> = hex
        .chars()
        .map(|symbol| symbol.to_digit(16).map(|digit| digit as u8))
        .collect::<Option<_>>()?;
    match digits.as_slice() {
        [red, green, blue] => Some(Color::rgb(red * 17, green * 17, blue * 17)),
        [red, green, blue, alpha] => Some(Color::rgba(red * 17, green * 17, blue * 17, alpha * 17)),
        [
            red_high,
            red_low,
            green_high,
            green_low,
            blue_high,
            blue_low,
        ] => Some(Color::rgb(
            red_high * 16 + red_low,
            green_high * 16 + green_low,
            blue_high * 16 + blue_low,
        )),
        [
            red_high,
            red_low,
            green_high,
            green_low,
            blue_high,
            blue_low,
            alpha_high,
            alpha_low,
        ] => Some(Color::rgba(
            red_high * 16 + red_low,
            green_high * 16 + green_low,
            blue_high * 16 + blue_low,
            alpha_high * 16 + alpha_low,
        )),
        _ => None,
    }
}

/// The spelling the parser reads back: a table name when one matches, else hex.
pub(crate) fn write(color: Color) -> String {
    if let Some((name, _)) = NAMED.iter().find(|(_, named)| *named == color) {
        return (*name).to_owned();
    }
    if color.is_opaque() {
        format!("#{:02x}{:02x}{:02x}", color.red, color.green, color.blue)
    } else {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            color.red, color.green, color.blue, color.alpha
        )
    }
}
