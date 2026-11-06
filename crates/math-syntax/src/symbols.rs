//! LaTeX command names to Unicode symbols, and the reverse for the writer.
//!
//! The codepoints follow unicode-math, which is what KaTeX's symbol tables
//! (MIT) encode; see `THIRD-PARTY.md`.

use math_core::{AtomClass, DelimiterSize, Limits, SpaceKind, Variant};

/// An accent command's spacing mark, as KaTeX draws it.
pub(crate) fn accent(name: &str) -> Option<char> {
    let symbol = match name {
        "hat" | "widehat" => '\u{02C6}',
        "tilde" | "widetilde" => '\u{02DC}',
        "bar" => '\u{00AF}',
        "vec" => '\u{20D7}',
        "dot" => '\u{02D9}',
        "ddot" => '\u{00A8}',
        "acute" => '\u{00B4}',
        "grave" => '\u{0060}',
        "breve" => '\u{02D8}',
        "check" => '\u{02C7}',
        "mathring" => '\u{02DA}',
        _ => return None,
    };
    Some(symbol)
}

pub(crate) fn accent_command(symbol: char) -> Option<&'static str> {
    let name = match symbol {
        '\u{02C6}' => "hat",
        '\u{02DC}' => "tilde",
        '\u{00AF}' => "bar",
        '\u{20D7}' => "vec",
        '\u{02D9}' => "dot",
        '\u{00A8}' => "ddot",
        '\u{00B4}' => "acute",
        '\u{0060}' => "grave",
        '\u{02D8}' => "breve",
        '\u{02C7}' => "check",
        '\u{02DA}' => "mathring",
        _ => return None,
    };
    Some(name)
}

pub(crate) fn font_command(name: &str) -> Option<Variant> {
    let variant = match name {
        "mathrm" => Variant::Upright,
        "mathbf" => Variant::Bold,
        "mathit" => Variant::Italic,
        "mathsf" => Variant::SansSerif,
        "mathtt" => Variant::Monospace,
        "mathcal" => Variant::Script,
        "mathbb" => Variant::DoubleStruck,
        "mathfrak" => Variant::Fraktur,
        "boldsymbol" => Variant::BoldItalic,
        "mathnormal" => Variant::Normal,
        _ => return None,
    };
    Some(variant)
}

pub(crate) fn negate(symbol: char) -> Option<char> {
    let negated = match symbol {
        '=' => '≠',
        '∈' => '∉',
        '<' => '≮',
        '>' => '≯',
        '≤' => '≰',
        '≥' => '≱',
        '⊂' => '⊄',
        '⊃' => '⊅',
        '⊆' => '⊈',
        '⊇' => '⊉',
        '≈' => '≉',
        '≡' => '≢',
        '∼' => '≁',
        '∣' => '∤',
        _ => return None,
    };
    Some(negated)
}

pub(crate) fn space(name: &str) -> Option<SpaceKind> {
    let kind = match name {
        "," | "thinspace" => SpaceKind::Thin,
        ":" | "medspace" => SpaceKind::Medium,
        ";" | "thickspace" => SpaceKind::Thick,
        "!" | "negthinspace" => SpaceKind::NegativeThin,
        "enspace" => SpaceKind::Half,
        "quad" => SpaceKind::Quad,
        "qquad" => SpaceKind::DoubleQuad,
        " " => SpaceKind::Normal,
        _ => return None,
    };
    Some(kind)
}

pub(crate) fn space_command(kind: SpaceKind) -> &'static str {
    match kind {
        SpaceKind::Thin => ",",
        SpaceKind::Medium => ":",
        SpaceKind::Thick => ";",
        SpaceKind::NegativeThin => "!",
        SpaceKind::Half => "enspace",
        SpaceKind::Quad => "quad",
        SpaceKind::DoubleQuad => "qquad",
        SpaceKind::Normal => " ",
    }
}

/// The `\big` family: a fixed size and the spacing class its suffix selects.
pub(crate) fn sized_delimiter(name: &str) -> Option<(DelimiterSize, AtomClass)> {
    let (size, suffix) = if let Some(suffix) = name.strip_prefix("Bigg") {
        (DelimiterSize::Biggest, suffix)
    } else if let Some(suffix) = name.strip_prefix("bigg") {
        (DelimiterSize::BiggerStill, suffix)
    } else if let Some(suffix) = name.strip_prefix("Big") {
        (DelimiterSize::Bigger, suffix)
    } else {
        let suffix = name.strip_prefix("big")?;
        (DelimiterSize::Big, suffix)
    };
    let class = match suffix {
        "" => AtomClass::Ord,
        "l" => AtomClass::Open,
        "r" => AtomClass::Close,
        "m" => AtomClass::Rel,
        _ => return None,
    };
    Some((size, class))
}

pub(crate) fn sized_delimiter_command(
    size: DelimiterSize,
    class: AtomClass,
) -> Option<&'static str> {
    let command = match (size, class) {
        (DelimiterSize::Big, AtomClass::Ord) => "big",
        (DelimiterSize::Big, AtomClass::Open) => "bigl",
        (DelimiterSize::Big, AtomClass::Close) => "bigr",
        (DelimiterSize::Big, AtomClass::Rel) => "bigm",
        (DelimiterSize::Bigger, AtomClass::Ord) => "Big",
        (DelimiterSize::Bigger, AtomClass::Open) => "Bigl",
        (DelimiterSize::Bigger, AtomClass::Close) => "Bigr",
        (DelimiterSize::Bigger, AtomClass::Rel) => "Bigm",
        (DelimiterSize::BiggerStill, AtomClass::Ord) => "bigg",
        (DelimiterSize::BiggerStill, AtomClass::Open) => "biggl",
        (DelimiterSize::BiggerStill, AtomClass::Close) => "biggr",
        (DelimiterSize::BiggerStill, AtomClass::Rel) => "biggm",
        (DelimiterSize::Biggest, AtomClass::Ord) => "Bigg",
        (DelimiterSize::Biggest, AtomClass::Open) => "Biggl",
        (DelimiterSize::Biggest, AtomClass::Close) => "Biggr",
        (DelimiterSize::Biggest, AtomClass::Rel) => "Biggm",
        _ => return None,
    };
    Some(command)
}

/// The two table defaults. An explicit `\limits` or `\nolimits` is a suffix,
/// so it never appears here.
const SIDE_DEFAULT: Limits = Limits::Default {
    display_limits: false,
};
const DISPLAY_DEFAULT: Limits = Limits::Default {
    display_limits: true,
};

/// A named operator: its displayed text (TeX's `\limsup` prints "lim sup")
/// and its intrinsic placement. `\injlim` is amsmath's `\operatorname*`,
/// while `\plim` carries `\limits` itself and is above/below in every style.
pub(crate) fn function(name: &str) -> Option<(&'static str, Limits)> {
    let function = match name {
        "sin" => ("sin", SIDE_DEFAULT),
        "cos" => ("cos", SIDE_DEFAULT),
        "tan" => ("tan", SIDE_DEFAULT),
        "cot" => ("cot", SIDE_DEFAULT),
        "sec" => ("sec", SIDE_DEFAULT),
        "csc" => ("csc", SIDE_DEFAULT),
        "arcsin" => ("arcsin", SIDE_DEFAULT),
        "arccos" => ("arccos", SIDE_DEFAULT),
        "arctan" => ("arctan", SIDE_DEFAULT),
        "arg" => ("arg", SIDE_DEFAULT),
        "sinh" => ("sinh", SIDE_DEFAULT),
        "cosh" => ("cosh", SIDE_DEFAULT),
        "tanh" => ("tanh", SIDE_DEFAULT),
        "coth" => ("coth", SIDE_DEFAULT),
        "log" => ("log", SIDE_DEFAULT),
        "ln" => ("ln", SIDE_DEFAULT),
        "lg" => ("lg", SIDE_DEFAULT),
        "exp" => ("exp", SIDE_DEFAULT),
        "deg" => ("deg", SIDE_DEFAULT),
        "dim" => ("dim", SIDE_DEFAULT),
        "hom" => ("hom", SIDE_DEFAULT),
        "ker" => ("ker", SIDE_DEFAULT),
        "lim" => ("lim", DISPLAY_DEFAULT),
        "limsup" => ("lim sup", DISPLAY_DEFAULT),
        "liminf" => ("lim inf", DISPLAY_DEFAULT),
        "argmax" => ("arg max", DISPLAY_DEFAULT),
        "argmin" => ("arg min", DISPLAY_DEFAULT),
        "injlim" => ("inj lim", DISPLAY_DEFAULT),
        "projlim" => ("proj lim", DISPLAY_DEFAULT),
        "max" => ("max", DISPLAY_DEFAULT),
        "min" => ("min", DISPLAY_DEFAULT),
        "sup" => ("sup", DISPLAY_DEFAULT),
        "inf" => ("inf", DISPLAY_DEFAULT),
        "det" => ("det", DISPLAY_DEFAULT),
        "gcd" => ("gcd", DISPLAY_DEFAULT),
        "Pr" => ("Pr", DISPLAY_DEFAULT),
        "plim" => ("plim", Limits::AboveBelow),
        _ => return None,
    };
    Some(function)
}

/// The command and intrinsic placement a displayed operator name came from.
/// A caller that attaches a different explicit placement writes it as a
/// suffix; a different default placement cannot use the named command.
pub(crate) fn function_command(name: &str) -> Option<(&'static str, Limits)> {
    let command = match name {
        "sin" => ("sin", SIDE_DEFAULT),
        "cos" => ("cos", SIDE_DEFAULT),
        "tan" => ("tan", SIDE_DEFAULT),
        "cot" => ("cot", SIDE_DEFAULT),
        "sec" => ("sec", SIDE_DEFAULT),
        "csc" => ("csc", SIDE_DEFAULT),
        "arcsin" => ("arcsin", SIDE_DEFAULT),
        "arccos" => ("arccos", SIDE_DEFAULT),
        "arctan" => ("arctan", SIDE_DEFAULT),
        "arg" => ("arg", SIDE_DEFAULT),
        "sinh" => ("sinh", SIDE_DEFAULT),
        "cosh" => ("cosh", SIDE_DEFAULT),
        "tanh" => ("tanh", SIDE_DEFAULT),
        "coth" => ("coth", SIDE_DEFAULT),
        "log" => ("log", SIDE_DEFAULT),
        "ln" => ("ln", SIDE_DEFAULT),
        "lg" => ("lg", SIDE_DEFAULT),
        "exp" => ("exp", SIDE_DEFAULT),
        "deg" => ("deg", SIDE_DEFAULT),
        "dim" => ("dim", SIDE_DEFAULT),
        "hom" => ("hom", SIDE_DEFAULT),
        "ker" => ("ker", SIDE_DEFAULT),
        "lim" => ("lim", DISPLAY_DEFAULT),
        "lim sup" => ("limsup", DISPLAY_DEFAULT),
        "lim inf" => ("liminf", DISPLAY_DEFAULT),
        "arg max" => ("argmax", DISPLAY_DEFAULT),
        "arg min" => ("argmin", DISPLAY_DEFAULT),
        "inj lim" => ("injlim", DISPLAY_DEFAULT),
        "proj lim" => ("projlim", DISPLAY_DEFAULT),
        "max" => ("max", DISPLAY_DEFAULT),
        "min" => ("min", DISPLAY_DEFAULT),
        "sup" => ("sup", DISPLAY_DEFAULT),
        "inf" => ("inf", DISPLAY_DEFAULT),
        "det" => ("det", DISPLAY_DEFAULT),
        "gcd" => ("gcd", DISPLAY_DEFAULT),
        "Pr" => ("Pr", DISPLAY_DEFAULT),
        "plim" => ("plim", Limits::AboveBelow),
        _ => return None,
    };
    Some(command)
}

pub(crate) fn variant_command(variant: Variant) -> Option<&'static str> {
    let name = match variant {
        Variant::Normal => return None,
        Variant::Upright => "mathrm",
        Variant::Bold => "mathbf",
        Variant::Italic => "mathit",
        Variant::BoldItalic => "boldsymbol",
        Variant::Script => "mathcal",
        Variant::Fraktur => "mathfrak",
        Variant::DoubleStruck => "mathbb",
        Variant::SansSerif => "mathsf",
        Variant::Monospace => "mathtt",
    };
    Some(name)
}

/// A symbol command usable as an atom.
pub(crate) fn command_char(name: &str) -> Option<char> {
    let symbol = match name {
        "alpha" => 'α',
        "beta" => 'β',
        "gamma" => 'γ',
        "delta" => 'δ',
        "epsilon" => 'ϵ',
        "varepsilon" => 'ε',
        "zeta" => 'ζ',
        "eta" => 'η',
        "theta" => 'θ',
        "vartheta" => 'ϑ',
        "iota" => 'ι',
        "kappa" => 'κ',
        "varkappa" => 'ϰ',
        "lambda" => 'λ',
        "mu" => 'μ',
        "nu" => 'ν',
        "xi" => 'ξ',
        "omicron" => 'ο',
        "pi" => 'π',
        "varpi" => 'ϖ',
        "rho" => 'ρ',
        "varrho" => 'ϱ',
        "sigma" => 'σ',
        "varsigma" => 'ς',
        "tau" => 'τ',
        "upsilon" => 'υ',
        "phi" => 'ϕ',
        "varphi" => 'φ',
        "chi" => 'χ',
        "psi" => 'ψ',
        "omega" => 'ω',
        "Gamma" => 'Γ',
        "Delta" => 'Δ',
        "Theta" => 'Θ',
        "Lambda" => 'Λ',
        "Xi" => 'Ξ',
        "Pi" => 'Π',
        "Sigma" => 'Σ',
        "Upsilon" => 'Υ',
        "Phi" => 'Φ',
        "Psi" => 'Ψ',
        "Omega" => 'Ω',
        "pm" => '±',
        "mp" => '∓',
        "times" => '×',
        "div" => '÷',
        "cdot" => '⋅',
        "ast" => '∗',
        "star" => '⋆',
        "circ" => '∘',
        "bullet" => '∙',
        "cap" => '∩',
        "cup" => '∪',
        "setminus" => '∖',
        "oplus" => '⊕',
        "ominus" => '⊖',
        "otimes" => '⊗',
        "oslash" => '⊘',
        "odot" => '⊙',
        "wedge" | "land" => '∧',
        "vee" | "lor" => '∨',
        "le" | "leq" => '≤',
        "ge" | "geq" => '≥',
        "ne" | "neq" => '≠',
        "approx" => '≈',
        "equiv" => '≡',
        "sim" => '∼',
        "simeq" => '≃',
        "cong" => '≅',
        "propto" => '∝',
        "in" => '∈',
        "notin" => '∉',
        "ni" => '∋',
        "subset" => '⊂',
        "supset" => '⊃',
        "subseteq" => '⊆',
        "supseteq" => '⊇',
        "to" | "rightarrow" => '→',
        "leftarrow" | "gets" => '←',
        "leftrightarrow" => '↔',
        "Rightarrow" | "implies" => '⇒',
        "Leftarrow" => '⇐',
        "Leftrightarrow" | "iff" => '⇔',
        "mapsto" => '↦',
        "mid" => '∣',
        "parallel" => '∥',
        "perp" => '⊥',
        "infty" => '∞',
        "partial" => '∂',
        "nabla" => '∇',
        "forall" => '∀',
        "exists" => '∃',
        "nexists" => '∄',
        "emptyset" | "varnothing" => '∅',
        "neg" | "lnot" => '¬',
        "prime" => '′',
        "ell" => 'ℓ',
        "hbar" => 'ℏ',
        "aleph" => 'ℵ',
        "angle" => '∠',
        "colon" => ':',
        "ldots" | "dots" | "dotsc" | "dotso" => '…',
        "cdots" | "dotsb" | "dotsm" | "dotsi" => '⋯',
        "vdots" => '⋮',
        "ddots" => '⋱',
        "{" | "lbrace" => '{',
        "}" | "rbrace" => '}',
        "%" => '%',
        "$" => '$',
        "&" => '&',
        "#" => '#',
        "_" => '_',
        "backslash" => '\\',
        "langle" => '⟨',
        "rangle" => '⟩',
        "lfloor" => '⌊',
        "rfloor" => '⌋',
        "lceil" => '⌈',
        "rceil" => '⌉',
        "vert" => '|',
        "Vert" => '‖',
        "uparrow" => '↑',
        "downarrow" => '↓',
        "updownarrow" => '↕',
        _ => return None,
    };
    Some(symbol)
}

/// Operators that grow and, in display style, take limits by default.
pub(crate) fn large_op(name: &str) -> Option<(char, bool)> {
    let operator = match name {
        "sum" => ('∑', true),
        "prod" => ('∏', true),
        "coprod" => ('∐', true),
        "bigcup" => ('⋃', true),
        "bigcap" => ('⋂', true),
        "bigoplus" => ('⨁', true),
        "bigotimes" => ('⨂', true),
        "bigodot" => ('⨀', true),
        "bigvee" => ('⋁', true),
        "bigwedge" => ('⋀', true),
        "biguplus" => ('⨄', true),
        "bigsqcup" => ('⨆', true),
        "int" => ('∫', false),
        "iint" => ('∬', false),
        "iiint" => ('∭', false),
        "oint" => ('∮', false),
        _ => return None,
    };
    Some(operator)
}

/// A command that stands for one character inside `\text{…}`. Names are the
/// LaTeX text-mode escapes; unknown commands stay literal.
pub(crate) fn text_command(name: &str) -> Option<char> {
    let symbol = match name {
        "textbackslash" => '\\',
        "textbar" => '|',
        "textless" => '<',
        "textgreater" => '>',
        "textasciitilde" => '~',
        "textasciicircum" => '^',
        "textunderscore" => '_',
        "textbraceleft" => '{',
        "textbraceright" => '}',
        "textquotedblleft" => '“',
        "textquotedblright" => '”',
        "textquoteleft" => '‘',
        "textquoteright" => '’',
        "textendash" => '–',
        "textemdash" => '—',
        "textellipsis" | "ldots" | "dots" => '…',
        "cdots" => '⋯',
        "textdollar" => '$',
        "textpercent" => '%',
        "textampersand" => '&',
        "textnumbersign" => '#',
        _ => return None,
    };
    Some(symbol)
}

/// A delimiter command; `.` as a literal symbol is the null delimiter.
pub(crate) fn delimiter(name: &str) -> Option<char> {
    match name {
        "{" | "lbrace" => Some('{'),
        "}" | "rbrace" => Some('}'),
        "langle" => Some('⟨'),
        "rangle" => Some('⟩'),
        "lfloor" => Some('⌊'),
        "rfloor" => Some('⌋'),
        "lceil" => Some('⌈'),
        "rceil" => Some('⌉'),
        "vert" | "lvert" | "rvert" => Some('|'),
        "Vert" | "lVert" | "rVert" | "|" => Some('‖'),
        "backslash" => Some('\\'),
        "uparrow" => Some('↑'),
        "downarrow" => Some('↓'),
        "updownarrow" => Some('↕'),
        "Uparrow" => Some('⇑'),
        "Downarrow" => Some('⇓'),
        "Updownarrow" => Some('⇕'),
        _ => None,
    }
}

pub(crate) fn large_op_command(symbol: char) -> Option<&'static str> {
    let name = match symbol {
        '∑' => "sum",
        '∏' => "prod",
        '∐' => "coprod",
        '⋃' => "bigcup",
        '⋂' => "bigcap",
        '⨁' => "bigoplus",
        '⨂' => "bigotimes",
        '⨀' => "bigodot",
        '⋁' => "bigvee",
        '⋀' => "bigwedge",
        '⨄' => "biguplus",
        '⨆' => "bigsqcup",
        '∫' => "int",
        '∬' => "iint",
        '∭' => "iiint",
        '∮' => "oint",
        _ => return None,
    };
    Some(name)
}

/// Delimiters whose LaTeX form is a command rather than the bare character.
pub(crate) fn delimiter_command(symbol: char) -> Option<&'static str> {
    let name = match symbol {
        '{' => "{",
        '}' => "}",
        '⟨' => "langle",
        '⟩' => "rangle",
        '⌊' => "lfloor",
        '⌋' => "rfloor",
        '⌈' => "lceil",
        '⌉' => "rceil",
        '‖' => "|",
        '\\' => "backslash",
        '↑' => "uparrow",
        '↓' => "downarrow",
        '↕' => "updownarrow",
        '⇑' => "Uparrow",
        '⇓' => "Downarrow",
        '⇕' => "Updownarrow",
        _ => return None,
    };
    Some(name)
}

/// Characters that carry syntax and must be escaped when written as atoms.
pub(crate) fn escape(symbol: char) -> Option<&'static str> {
    let text = match symbol {
        '{' => "\\{",
        '}' => "\\}",
        '%' => "\\%",
        '$' => "\\$",
        '&' => "\\&",
        '#' => "\\#",
        '_' => "\\_",
        '\\' => "\\backslash",
        _ => return None,
    };
    Some(text)
}
