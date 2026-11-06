//! A leaf symbol.

use crate::class::{AtomClass, class_of};

/// The alphabet a symbol is drawn from. `Normal` is math italic for Latin
/// letters; the others select the Unicode Mathematical Alphanumeric block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Variant {
    Normal,
    Upright,
    Bold,
    Italic,
    BoldItalic,
    Script,
    Fraktur,
    DoubleStruck,
    SansSerif,
    Monospace,
}

/// One symbol with the class that decides the glue around it.
///
/// The model stores the character, not a glyph: the same document must lay out
/// against any font, and glyph selection (variant, math italic, `ssty`) is a
/// layout decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Atom {
    pub symbol: char,
    pub class: AtomClass,
    pub variant: Variant,
}

impl Atom {
    /// Classifies `symbol` with the symbol table.
    pub fn new(symbol: char) -> Self {
        Atom {
            symbol,
            class: class_of(symbol),
            variant: Variant::Normal,
        }
    }

    /// Classifies `symbol` and draws it in `variant`.
    pub fn with_variant(symbol: char, variant: Variant) -> Self {
        Atom {
            symbol,
            class: class_of(symbol),
            variant,
        }
    }

    /// Classifies `symbol` with an explicit class, for commands whose class
    /// is not derivable from the codepoint.
    pub fn with_class(symbol: char, class: AtomClass) -> Self {
        Atom {
            symbol,
            class,
            variant: Variant::Normal,
        }
    }
}

/// The Mathematical Italic codepoint for an ASCII letter.
///
/// U+1D455 is unassigned: small h is the Planck constant at U+210E.
pub fn math_italic(symbol: char) -> Option<char> {
    match symbol {
        'A'..='Z' => char::from_u32(0x1D434 + (symbol as u32 - 'A' as u32)),
        'a'..='z' if symbol != 'h' => char::from_u32(0x1D44E + (symbol as u32 - 'a' as u32)),
        'h' => Some('\u{210E}'),
        _ => None,
    }
}

/// The codepoint for `symbol` in `variant`, when Unicode has one.
///
/// Several script, fraktur and double-struck letters live outside their block
/// in Letterlike Symbols, so those slots are mapped explicitly rather than
/// returning an unassigned codepoint. Greek and symbols without a variant
/// return `None`; layout falls back to the base character.
pub fn variant_char(symbol: char, variant: Variant) -> Option<char> {
    match variant {
        Variant::Normal | Variant::Upright => None,
        Variant::Bold => block(symbol, 0x1D400, 0x1D41A, Some(0x1D7CE)),
        Variant::Italic => match symbol {
            'h' => Some('\u{210E}'),
            _ => block(symbol, 0x1D434, 0x1D44E, None),
        },
        Variant::BoldItalic => block(symbol, 0x1D468, 0x1D482, None),
        Variant::Script => script(symbol),
        Variant::Fraktur => fraktur(symbol),
        Variant::DoubleStruck => double_struck(symbol),
        Variant::SansSerif => block(symbol, 0x1D5A0, 0x1D5BA, Some(0x1D7E2)),
        Variant::Monospace => block(symbol, 0x1D670, 0x1D68A, Some(0x1D7F6)),
    }
}

fn block(symbol: char, upper: u32, lower: u32, digits: Option<u32>) -> Option<char> {
    match symbol {
        'A'..='Z' => char::from_u32(upper + (symbol as u32 - 'A' as u32)),
        'a'..='z' => char::from_u32(lower + (symbol as u32 - 'a' as u32)),
        '0'..='9' => {
            let base = digits?;
            char::from_u32(base + (symbol as u32 - '0' as u32))
        }
        _ => None,
    }
}

fn script(symbol: char) -> Option<char> {
    let letter = match symbol {
        'B' => '\u{212C}',
        'E' => '\u{2130}',
        'F' => '\u{2131}',
        'H' => '\u{210B}',
        'I' => '\u{2110}',
        'L' => '\u{2112}',
        'M' => '\u{2133}',
        'R' => '\u{211B}',
        'e' => '\u{212F}',
        'g' => '\u{210A}',
        'o' => '\u{2134}',
        'A'..='Z' => return char::from_u32(0x1D49C + (symbol as u32 - 'A' as u32)),
        'a'..='z' => return char::from_u32(0x1D4B6 + (symbol as u32 - 'a' as u32)),
        _ => return None,
    };
    Some(letter)
}

fn fraktur(symbol: char) -> Option<char> {
    let letter = match symbol {
        'C' => '\u{212D}',
        'H' => '\u{210C}',
        'I' => '\u{2111}',
        'R' => '\u{211C}',
        'Z' => '\u{2128}',
        'A'..='Z' => return char::from_u32(0x1D504 + (symbol as u32 - 'A' as u32)),
        'a'..='z' => return char::from_u32(0x1D51E + (symbol as u32 - 'a' as u32)),
        _ => return None,
    };
    Some(letter)
}

fn double_struck(symbol: char) -> Option<char> {
    let letter = match symbol {
        'C' => '\u{2102}',
        'H' => '\u{210D}',
        'N' => '\u{2115}',
        'P' => '\u{2119}',
        'Q' => '\u{211A}',
        'R' => '\u{211D}',
        'Z' => '\u{2124}',
        'A'..='Z' => return char::from_u32(0x1D538 + (symbol as u32 - 'A' as u32)),
        'a'..='z' => return char::from_u32(0x1D552 + (symbol as u32 - 'a' as u32)),
        '0'..='9' => return char::from_u32(0x1D7D8 + (symbol as u32 - '0' as u32)),
        _ => return None,
    };
    Some(letter)
}
