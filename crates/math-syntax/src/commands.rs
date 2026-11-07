//! What a command expects after its name.
//!
//! The parser's match arms know the grammar; this table names the shapes so a
//! host can decide how to consume the next key without keeping a second list.

use crate::symbols;

/// The tokens a command expects after its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandShape {
    /// No argument: a symbol command, large operator, function, spacing, or a
    /// no-argument construct such as `\mathstrut`.
    None,
    /// The next token is a delimiter (`\left`, `\right`, `\middle`, `\bigl`).
    Delimiter,
    /// One braced math argument (`\hat`, `\overline`, `\cancel`).
    MathArgument,
    /// One math argument whose atoms take a font variant (`\mathbb`).
    VariantArgument,
    /// Two braced math arguments (`\frac`, `\overset`).
    TwoMathArguments,
    /// An optional `[...]` degree, then one math argument (`\sqrt`).
    OptionalMathArgument,
    /// An optional `[t|b]` flag, then one math argument (`\smash`).
    OptionalFlagArgument,
    /// An optional `[...]` under-script, then one math argument
    /// (`\xrightarrow`).
    OptionalUnderArgument,
    /// A braced name or text argument (`\operatorname`, `\text`).
    NameArgument,
    /// A braced environment name (`\begin`).
    Environment,
    /// A braced length (`\hspace`).
    Length,
    /// The next token is a relation symbol (`\not`).
    SymbolArgument,
    /// A command whose arguments the generic template cannot express
    /// (`\genfrac`, `\substack`, `\pmod`, `\colorbox`, `\mathchoice`).
    Special,
    Unknown,
}

/// Classifies `name` as the parser would read it.
pub fn command_shape(name: &str) -> CommandShape {
    match name {
        "begin" => CommandShape::Environment,
        "frac" | "dfrac" | "tfrac" | "cfrac" | "binom" | "dbinom" | "tbinom" | "overset"
        | "underset" | "stackrel" => CommandShape::TwoMathArguments,
        "sqrt" => CommandShape::OptionalMathArgument,
        "smash" => CommandShape::OptionalFlagArgument,
        "xrightarrow" | "xleftarrow" => CommandShape::OptionalUnderArgument,
        "text" | "operatorname" | "operatornamewithlimits" => CommandShape::NameArgument,
        "hspace" => CommandShape::Length,
        "genfrac" | "substack" | "pmod" | "color" | "textcolor" | "colorbox" | "fcolorbox"
        | "mathchoice" => CommandShape::Special,
        "not" => CommandShape::SymbolArgument,
        "mod" | "pod" => CommandShape::MathArgument,
        "left" | "right" | "middle" => CommandShape::Delimiter,
        "mathord" | "mathop" | "mathbin" | "mathrel" | "mathopen" | "mathclose" | "mathpunct"
        | "mathinner" | "underrightarrow" | "underleftarrow" => CommandShape::MathArgument,
        "varliminf" | "varlimsup" | "varinjlim" | "varprojlim" | "mathstrut" => CommandShape::None,
        "overline" | "underline" | "phantom" | "hphantom" | "vphantom" | "cancel" | "bcancel"
        | "xcancel" | "boxed" | "overbrace" | "underbrace" => CommandShape::MathArgument,
        _ => {
            if symbols::font_command(name).is_some() {
                CommandShape::VariantArgument
            } else if symbols::accent(name).is_some() {
                CommandShape::MathArgument
            } else if symbols::sized_delimiter(name).is_some() {
                CommandShape::Delimiter
            } else if symbols::space(name).is_some()
                || symbols::function(name).is_some()
                || symbols::large_op(name).is_some()
                || symbols::command_char(name).is_some()
                || name == "bmod"
            {
                CommandShape::None
            } else {
                CommandShape::Unknown
            }
        }
    }
}

/// The delimiter a command name stands for; `|` is the double bar as `\|` is.
pub fn delimiter_symbol(name: &str) -> Option<char> {
    symbols::delimiter(name)
}

/// The empty-argument snippet a host may insert for `name`, when the shape
/// allows one. A name argument still needs model support the editor does not
/// have, so it deliberately returns `None`.
pub fn command_template(name: &str) -> Option<String> {
    match command_shape(name) {
        CommandShape::TwoMathArguments => Some(format!("\\{name}{{}}{{}}")),
        CommandShape::NameArgument if name == "text" => Some("\\text{}".to_owned()),
        CommandShape::NameArgument if matches!(name, "operatorname" | "operatornamewithlimits") => {
            Some(format!("\\{name}{{}}"))
        }
        CommandShape::Special if name == "pmod" => Some("\\pmod{}".to_owned()),
        CommandShape::MathArgument
        | CommandShape::VariantArgument
        | CommandShape::OptionalMathArgument
        | CommandShape::OptionalFlagArgument
        | CommandShape::OptionalUnderArgument => Some(format!("\\{name}{{}}")),
        _ => None,
    }
}
