//! What a command expects after its name.
//!
//! The parser's match arms know the grammar; this table names the shapes so a
//! host can decide how to consume the next key without keeping a second list.

use crate::symbols;

/// The tokens a command expects after its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandShape {
    /// No argument: a symbol command, large operator, function or spacing.
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
    /// (`\genfrac`, `\substack`, `\pmod`).
    Special,
    Unknown,
}

/// Classifies `name` as the parser would read it.
pub fn command_shape(name: &str) -> CommandShape {
    match name {
        "begin" => CommandShape::Environment,
        "frac" | "dfrac" | "tfrac" | "cfrac" | "overset" | "underset" | "stackrel" => {
            CommandShape::TwoMathArguments
        }
        "sqrt" => CommandShape::OptionalMathArgument,
        "smash" => CommandShape::OptionalFlagArgument,
        "xrightarrow" | "xleftarrow" => CommandShape::OptionalUnderArgument,
        "text" | "operatorname" => CommandShape::NameArgument,
        "hspace" => CommandShape::Length,
        "genfrac" | "substack" | "pmod" => CommandShape::Special,
        "not" => CommandShape::SymbolArgument,
        "left" | "right" | "middle" => CommandShape::Delimiter,
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

/// The empty-argument snippet a host may insert for `name`, when the shape
/// allows one. Variant and name arguments need model support the editor does
/// not have yet, so they deliberately return `None`.
pub fn command_template(name: &str) -> Option<String> {
    match command_shape(name) {
        CommandShape::TwoMathArguments => Some(format!("\\{name}{{}}{{}}")),
        CommandShape::MathArgument
        | CommandShape::OptionalMathArgument
        | CommandShape::OptionalFlagArgument
        | CommandShape::OptionalUnderArgument => Some(format!("\\{name}{{}}")),
        _ => None,
    }
}
