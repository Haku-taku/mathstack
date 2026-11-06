//! Atom classes and the TeX inter-atom spacing tables.

/// The eight classes that determine spacing (TeXbook p. 158).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AtomClass {
    Ord = 0,
    Op = 1,
    Bin = 2,
    Rel = 3,
    Open = 4,
    Close = 5,
    Punct = 6,
    Inner = 7,
}

/// One spacing cell, in multiples of 1/18 em.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gap {
    None,
    Thin,
    Medium,
    Thick,
}

impl Gap {
    /// Width in em; TeX's mu is 1/18 em.
    pub fn em(self) -> f32 {
        match self {
            Gap::None => 0.0,
            Gap::Thin => 3.0 / 18.0,
            Gap::Medium => 4.0 / 18.0,
            Gap::Thick => 5.0 / 18.0,
        }
    }
}

const NONE: Gap = Gap::None;
const THIN: Gap = Gap::Thin;
const MEDIUM: Gap = Gap::Medium;
const THICK: Gap = Gap::Thick;

/// Rows are the left atom, columns the right one (TeXbook p. 170).
///
/// Copied from KaTeX `src/spacingData.ts` (MIT), whose table is the same data;
/// the medium and thick cells are the ones TeX wraps in `\nonscript`.
const WIDE: [[Gap; 8]; 8] = [
    //          Ord      Op      Bin      Rel      Open     Close    Punct    Inner
    /* Ord */
    [NONE, THIN, MEDIUM, THICK, NONE, NONE, NONE, THIN],
    /* Op  */ [THIN, THIN, NONE, THICK, NONE, NONE, NONE, THIN],
    /* Bin */ [MEDIUM, MEDIUM, NONE, NONE, MEDIUM, NONE, NONE, MEDIUM],
    /* Rel */ [THICK, THICK, NONE, NONE, THICK, NONE, NONE, THICK],
    /* Open*/ [NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE],
    /* Close*/ [NONE, THIN, MEDIUM, THICK, NONE, NONE, NONE, THIN],
    /* Punct*/ [THIN, THIN, NONE, THIN, THIN, THIN, THIN, THIN],
    /* Inner*/ [THIN, THIN, MEDIUM, THICK, THIN, NONE, THIN, THIN],
];

/// Script styles use KaTeX's `tightSpacings`: medium and thick glue vanish and
/// only these thin cells survive.
const TIGHT: [[Gap; 8]; 8] = [
    //          Ord      Op      Bin      Rel      Open     Close    Punct    Inner
    /* Ord */
    [NONE, THIN, NONE, NONE, NONE, NONE, NONE, NONE],
    /* Op  */ [THIN, THIN, NONE, NONE, NONE, NONE, NONE, NONE],
    /* Bin */ [NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE],
    /* Rel */ [NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE],
    /* Open*/ [NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE],
    /* Close*/ [NONE, THIN, NONE, NONE, NONE, NONE, NONE, NONE],
    /* Punct*/ [NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE],
    /* Inner*/ [THIN, THIN, NONE, NONE, THIN, NONE, THIN, THIN],
];

/// Glue between two adjacent atoms. `tight` selects the script-style table.
pub fn gap(left: AtomClass, right: AtomClass, tight: bool) -> Gap {
    let table = if tight { &TIGHT } else { &WIDE };
    table[left as usize][right as usize]
}

/// The spacing class of a symbol, from its Unicode value.
///
/// Complete for the symbol commands `math-syntax` accepts; anything else is
/// ordinary, which is also the right class for most letters and numerals.
pub fn class_of(symbol: char) -> AtomClass {
    match symbol {
        '+' | '-' | '*' | '/' | '±' | '∓' | '×' | '÷' | '·' | '⋅' | '∗' | '⋆' | '∘' | '∙' | '∪'
        | '∩' | '∖' | '∧' | '∨' | '⊕' | '⊖' | '⊗' | '⊘' | '⊙' => AtomClass::Bin,
        '=' | '<' | '>' | '≤' | '≥' | '≠' | '≈' | '≡' | '∼' | '≃' | '≅' | '∝' | '≮' | '≯' | '≰'
        | '≱' | '⊄' | '⊅' | '⊈' | '⊉' | '≉' | '≢' | '≁' | '∤' | '→' | '←' | '↔' | '↦' | '⇒'
        | '⇐' | '⇔' | '∈' | '∉' | '∋' | '⊂' | '⊃' | '⊆' | '⊇' | '∣' | '∥' | '⊥' | ':' => {
            AtomClass::Rel
        }
        '(' | '[' | '{' | '⟨' | '⌊' | '⌈' => AtomClass::Open,
        ')' | ']' | '}' | '⟩' | '⌋' | '⌉' => AtomClass::Close,
        ',' | ';' => AtomClass::Punct,
        '∑' | '∏' | '∐' | '∫' | '∬' | '∭' | '∮' | '⋃' | '⋂' | '⨁' | '⨂' | '⨀' | '⋁' | '⋀' | '⨄'
        | '⨆' => AtomClass::Op,
        _ => AtomClass::Ord,
    }
}
