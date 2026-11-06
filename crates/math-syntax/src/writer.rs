//! Model tree back to LaTeX.
//!
//! The output normalizes whitespace and redundant braces, so a parsed document
//! round-trips structurally rather than byte for byte. A barless fraction is
//! written with `\atop`, the infix form that parses back without a rule.

use crate::spellings::Spellings;
use crate::symbols;
use math_core::{
    Alignment, Atom, FractionRule, Length, LengthUnit, Limits, Model, NodeId, NodeKind, Origin,
    Size, Span,
};

/// Where a node sits: a nested row in an item position needs braces to stay a
/// row, while an argument's braces are supplied by its caller.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Context {
    Top,
    Item,
    Argument,
}

pub fn to_latex(model: &Model, root: NodeId) -> String {
    to_latex_with_spans(model, root).0
}

/// Writes the canonical form, except for nodes whose spelling is still known.
pub fn to_latex_with_spellings(model: &Model, root: NodeId, spellings: &Spellings) -> String {
    to_latex_with_spans_and_spellings(model, root, spellings).0
}

/// Writes the subtree and records where each node landed in the output.
///
/// The spans are byte offsets into the returned text, not into whatever source
/// produced the model: an editor that exports after every edit needs offsets
/// that stay valid as the text changes.
pub fn to_latex_with_spans(model: &Model, root: NodeId) -> (String, Vec<(NodeId, Span)>) {
    let spellings = Spellings::default();
    to_latex_with_spans_and_spellings(model, root, &spellings)
}

/// Writes the subtree and records spans, emitting original tokens where the
/// node still matches the shape its spelling was read from.
pub fn to_latex_with_spans_and_spellings(
    model: &Model,
    root: NodeId,
    spellings: &Spellings,
) -> (String, Vec<(NodeId, Span)>) {
    let mut out = Output {
        text: String::new(),
        spans: Vec::new(),
        spellings,
    };
    write(model, root, Context::Top, &mut out);
    (out.text, out.spans)
}

struct Output<'a> {
    text: String,
    spans: Vec<(NodeId, Span)>,
    spellings: &'a Spellings,
}

impl Output<'_> {
    fn push(&mut self, symbol: char) {
        self.text.push(symbol);
    }

    fn push_str(&mut self, text: &str) {
        self.text.push_str(text);
    }

    /// Appends a token, keeping an alphabetic command separated from what
    /// follows it.
    fn push_token(&mut self, text: &str) {
        self.text.push_str(text);
        if text.starts_with('\\')
            && text
                .chars()
                .last()
                .is_some_and(|last| last.is_ascii_alphabetic())
        {
            self.text.push(' ');
        }
    }

    fn spelling(&self, model: &Model, id: NodeId) -> Option<String> {
        let spelling = self.spellings.get(id)?;
        if spelling.matches(model.kind(id)) {
            Some(spelling.text.clone())
        } else {
            None
        }
    }
}

fn write(model: &Model, id: NodeId, context: Context, out: &mut Output<'_>) {
    let start = out.text.len();
    match model.kind(id) {
        NodeKind::Atom(atom) => match out.spelling(model, id) {
            Some(text) => out.push_token(&text),
            None => write_atom(atom, out),
        },
        NodeKind::Row(children) => {
            let braces = context == Context::Item;
            if braces {
                out.push('{');
            }
            for child in children {
                write(model, *child, Context::Item, out);
            }
            if braces {
                out.push('}');
            }
        }
        NodeKind::Scripts { base, sup, sub } => {
            write(model, *base, Context::Item, out);
            if let Some(sub) = sub {
                out.push('_');
                write_argument(model, *sub, out);
            }
            if let Some(sup) = sup {
                out.push('^');
                write_argument(model, *sup, out);
            }
        }
        NodeKind::Fraction {
            numerator,
            denominator,
            rule,
            style,
        } => match (rule, style) {
            (FractionRule::None, None) => {
                // The infix form; its row context supplies any braces.
                write(model, *numerator, Context::Argument, out);
                out.push_str(" \\atop ");
                write(model, *denominator, Context::Argument, out);
            }
            (FractionRule::Default, Some(Size::Display)) => {
                push_command(out, "dfrac");
                write_argument(model, *numerator, out);
                write_argument(model, *denominator, out);
            }
            (FractionRule::Default, Some(Size::Text)) => {
                push_command(out, "tfrac");
                write_argument(model, *numerator, out);
                write_argument(model, *denominator, out);
            }
            (FractionRule::Default, None) => {
                push_command(out, "frac");
                write_argument(model, *numerator, out);
                write_argument(model, *denominator, out);
            }
            (rule, style) => write_genfrac(model, *numerator, *denominator, *rule, *style, out),
        },
        NodeKind::Radical { body, degree } => {
            push_command(out, "sqrt");
            if let Some(degree) = degree {
                out.push('[');
                write(model, *degree, Context::Argument, out);
                out.push(']');
            }
            write_argument(model, *body, out);
        }
        NodeKind::Delimited { left, body, right } => {
            if let Some(table) = cases_table(model, *body, *left, *right) {
                write_environment(model, table, "cases", out);
            } else if let Some((command, numerator, denominator)) =
                binomial(model, *body, *left, *right)
            {
                push_command(out, command);
                write_argument(model, numerator, out);
                write_argument(model, denominator, out);
            } else {
                push_command(out, "left");
                write_delimiter(*left, out);
                write(model, *body, Context::Argument, out);
                push_command(out, "right");
                write_delimiter(*right, out);
            }
        }
        NodeKind::LargeOp { symbol, limits } => {
            match out.spelling(model, id) {
                Some(text) => out.push_token(&text),
                None => match symbols::large_op_command(*symbol) {
                    Some(name) => push_command(out, name),
                    None => write_symbol(*symbol, out),
                },
            }
            write_limits_suffix(*limits, out);
        }
        NodeKind::Accent { base, symbol } => {
            // Parsed accents always have a command; an API-built mark without
            // one falls back to the hat, since the writer has no diagnostics.
            match out.spelling(model, id) {
                Some(text) => out.push_token(&text),
                None => {
                    let name = symbols::accent_command(*symbol).unwrap_or("hat");
                    push_command(out, name);
                }
            }
            write_argument(model, *base, out);
        }
        NodeKind::Composition(_) => {
            // A live preedit is not part of the document; commit replaces it.
        }
        NodeKind::Text(text) => match out.spelling(model, id) {
            Some(token) => out.push_token(&token),
            None => {
                out.push_str("\\text{");
                write_text(text, out);
                out.push('}');
            }
        },
        NodeKind::Function { name, limits } => {
            match out.spelling(model, id) {
                Some(text) => out.push_token(&text),
                None => match symbols::function_command(name, *limits) {
                    Some(command) => push_command(out, command),
                    None => {
                        out.push_str("\\operatorname");
                        if matches!(
                            limits,
                            Limits::Default {
                                display_limits: true
                            }
                        ) {
                            out.push('*');
                        }
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                },
            }
            write_limits_suffix(*limits, out);
        }
        NodeKind::Space(kind) => match out.spelling(model, id) {
            Some(text) => out.push_token(&text),
            None => {
                out.push('\\');
                out.push_str(symbols::space_command(*kind));
                out.push(' ');
            }
        },
        NodeKind::Hspace(length) => {
            push_command(out, "hspace");
            out.push('{');
            write_length(*length, out);
            out.push('}');
        }
        NodeKind::Overline { body } => {
            push_command(out, "overline");
            write_argument(model, *body, out);
        }
        NodeKind::Underline { body } => {
            push_command(out, "underline");
            write_argument(model, *body, out);
        }
        NodeKind::Middle { symbol } => {
            match out.spelling(model, id) {
                Some(text) => out.push_token(&text),
                None => push_command(out, "middle"),
            }
            write_delimiter(Some(*symbol), out);
        }
        NodeKind::SizedDelimiter {
            symbol,
            size,
            class,
        } => {
            match out.spelling(model, id) {
                Some(text) => out.push_token(&text),
                None => {
                    if let Some(command) = symbols::sized_delimiter_command(*size, *class) {
                        push_command(out, command);
                    }
                }
            }
            write_delimiter(Some(*symbol), out);
        }
        NodeKind::Phantom { body, kind } => {
            push_command(
                out,
                match kind {
                    math_core::PhantomKind::Full => "phantom",
                    math_core::PhantomKind::Horizontal => "hphantom",
                    math_core::PhantomKind::Vertical => "vphantom",
                },
            );
            write_argument(model, *body, out);
        }
        NodeKind::Smash { body, top, bottom } => {
            push_command(out, "smash");
            if *top && !*bottom {
                out.push_str("[t]");
            } else if !*top && *bottom {
                out.push_str("[b]");
            }
            write_argument(model, *body, out);
        }
        NodeKind::Boxed { body } => {
            push_command(out, "boxed");
            write_argument(model, *body, out);
        }
        NodeKind::Arrow {
            symbol,
            over,
            under,
        } => {
            push_command(
                out,
                match symbol {
                    '\u{2190}' => "xleftarrow",
                    _ => "xrightarrow",
                },
            );
            if let Some(under) = under {
                out.push('[');
                write(model, *under, Context::Argument, out);
                out.push(']');
            }
            match over {
                Some(over) => write_argument(model, *over, out),
                None => out.push_str("{}"),
            }
        }
        NodeKind::HorizontalBrace { body, over } => {
            push_command(out, if *over { "overbrace" } else { "underbrace" });
            write_argument(model, *body, out);
        }
        NodeKind::Cancel { body, up, down } => {
            push_command(
                out,
                match (*up, *down) {
                    (true, true) => "xcancel",
                    (true, false) => "cancel",
                    (false, true) => "bcancel",
                    (false, false) => "",
                },
            );
            write_argument(model, *body, out);
        }
        NodeKind::OverUnder { base, over, under } => {
            if let Some(over) = over {
                push_command(out, "overset");
                write_argument(model, *over, out);
                write_argument(model, *base, out);
            } else if let Some(under) = under {
                push_command(out, "underset");
                write_argument(model, *under, out);
                write_argument(model, *base, out);
            } else {
                write(model, *base, Context::Item, out);
            }
        }
        NodeKind::Table {
            columns, alignment, ..
        } => {
            let name = if aligned_columns(*columns, alignment) {
                "aligned"
            } else {
                "matrix"
            };
            write_environment(model, id, name, out)
        }
        NodeKind::TableRow { cells } => {
            for (index, cell) in cells.iter().enumerate() {
                if index > 0 {
                    out.push_str(" & ");
                }
                write(model, *cell, Context::Argument, out);
            }
        }
        NodeKind::Error(text) => {
            out.push_str(text);
            out.push(' ');
        }
    }
    out.spans.push((id, Span::new(start, out.text.len())));
}

/// `\binom`, `\dbinom` or `\tbinom` for the delimited ruleless fraction they
/// expand to; other ruleless fractions keep their `\left(…\right)` form.
fn binomial(
    model: &Model,
    body: NodeId,
    left: Option<char>,
    right: Option<char>,
) -> Option<(&'static str, NodeId, NodeId)> {
    if left != Some('(') || right != Some(')') {
        return None;
    }
    let NodeKind::Row(content) = model.kind(body) else {
        return None;
    };
    let [fraction] = content.as_slice() else {
        return None;
    };
    let NodeKind::Fraction {
        numerator,
        denominator,
        rule: FractionRule::None,
        style,
    } = model.kind(*fraction)
    else {
        return None;
    };
    let command = match style {
        None => "binom",
        Some(Size::Display) => "dbinom",
        Some(Size::Text) => "tbinom",
        Some(Size::Script | Size::ScriptScript) => return None,
    };
    Some((command, *numerator, *denominator))
}

fn write_limits_suffix(limits: Limits, out: &mut Output<'_>) {
    match limits {
        Limits::Default { .. } => {}
        Limits::AboveBelow => push_command(out, "limits"),
        Limits::Beside => push_command(out, "nolimits"),
    }
}

/// `\begin{name} rows \end{name}` for a table node.
/// `\genfrac` covers the combinations the named commands cannot spell: a
/// custom rule, or a script-level style.
fn write_genfrac(
    model: &Model,
    numerator: NodeId,
    denominator: NodeId,
    rule: FractionRule,
    style: Option<Size>,
    out: &mut Output<'_>,
) {
    push_command(out, "genfrac");
    out.push_str("{}{}");
    out.push('{');
    match rule {
        FractionRule::None => out.push_str("0em"),
        FractionRule::Default => {}
        FractionRule::Custom(length) => write_length(length, out),
    }
    out.push_str("}{");
    if let Some(size) = style {
        out.push_str(match size {
            Size::Display => "0",
            Size::Text => "1",
            Size::Script => "2",
            Size::ScriptScript => "3",
        });
    }
    out.push('}');
    write_argument(model, numerator, out);
    write_argument(model, denominator, out);
}

fn write_length(length: Length, out: &mut Output<'_>) {
    let milli = length.milli;
    if milli % 1000 == 0 {
        out.push_str(&(milli / 1000).to_string());
    } else {
        let sign = if milli < 0 { "-" } else { "" };
        let magnitude = milli.unsigned_abs();
        let mut fraction = format!("{:03}", magnitude % 1000);
        while fraction.ends_with('0') {
            fraction.pop();
        }
        out.push_str(&format!("{sign}{}.{fraction}", magnitude / 1000));
    }
    out.push_str(match length.unit {
        LengthUnit::Em => "em",
        LengthUnit::Ex => "ex",
        LengthUnit::Mu => "mu",
    });
}

/// Whether the columns read as an `aligned` block: a right-aligned column,
/// then a left-aligned one, repeating.
fn aligned_columns(columns: usize, alignment: &[Alignment]) -> bool {
    columns > 0
        && alignment.len() == columns
        && alignment.iter().enumerate().all(|(column, align)| {
            *align
                == if column % 2 == 0 {
                    Alignment::Right
                } else {
                    Alignment::Left
                }
        })
}

fn write_environment(model: &Model, table: NodeId, name: &str, out: &mut Output<'_>) {
    let NodeKind::Table { rows, .. } = model.kind(table) else {
        return;
    };
    push_command(out, "begin");
    out.push('{');
    out.push_str(name);
    out.push('}');
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            out.push_str(" \\\\ ");
        }
        write(model, *row, Context::Argument, out);
    }
    push_command(out, "end");
    out.push('{');
    out.push_str(name);
    out.push('}');
}

/// A left brace with no right fence and left-aligned columns is `cases`; the
/// environment name is the only place the alignment survives, so the writer
/// must emit it to round-trip the model.
fn cases_table(
    model: &Model,
    body: NodeId,
    left: Option<char>,
    right: Option<char>,
) -> Option<NodeId> {
    if left != Some('{') || right.is_some() {
        return None;
    }
    let NodeKind::Row(content) = model.kind(body) else {
        return None;
    };
    let [table] = content.as_slice() else {
        return None;
    };
    match model.kind(*table) {
        NodeKind::Table { alignment, .. }
            if !alignment.is_empty()
                && alignment
                    .iter()
                    .all(|column| *column == math_core::Alignment::Left) =>
        {
            Some(*table)
        }
        _ => None,
    }
}

/// The parser unwraps a one-child braced argument; a second pair of braces
/// keeps a genuine one-item row from collapsing on the way back in.
///
/// A synthetic row is an editor's typing container, not a group the source
/// wrote, so its braces must not multiply.
fn write_argument(model: &Model, id: NodeId, out: &mut Output<'_>) {
    let single_row = matches!(model.kind(id), NodeKind::Row(children) if children.len() == 1)
        && !matches!(model.node(id).origin, Origin::Synthetic);
    if single_row {
        out.push('{');
    }
    out.push('{');
    write(model, id, Context::Argument, out);
    out.push('}');
    if single_row {
        out.push('}');
    }
}

fn write_atom(atom: &Atom, out: &mut Output<'_>) {
    match symbols::variant_command(atom.variant) {
        Some(name) => {
            push_command(out, name);
            out.push('{');
            write_symbol(atom.symbol, out);
            out.push('}');
        }
        None => write_symbol(atom.symbol, out),
    }
}

/// Escapes semantic text back into text mode; grouping braces never reach the
/// node, so braces here are literal.
fn write_text(text: &str, out: &mut Output<'_>) {
    for symbol in text.chars() {
        match symbol {
            '{' | '}' | '%' | '$' | '&' | '#' | '_' => {
                out.push('\\');
                out.push(symbol);
            }
            '\\' => out.push_str("\\textbackslash{}"),
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            _ => out.push(symbol),
        }
    }
}

fn write_symbol(symbol: char, out: &mut Output<'_>) {
    match symbols::escape(symbol) {
        Some(text) => {
            out.push_str(text);
            out.push(' ');
        }
        None => out.push(symbol),
    }
}

fn write_delimiter(symbol: Option<char>, out: &mut Output<'_>) {
    let Some(symbol) = symbol else {
        out.push('.');
        return;
    };
    match symbols::delimiter_command(symbol) {
        Some(name) => push_command(out, name),
        None => out.push(symbol),
    }
}

/// A command followed by a space: the lexer ends an alphabetic command only at
/// a non-letter, so `\suma` would otherwise read as one unknown command.
fn push_command(out: &mut Output<'_>, name: &str) {
    out.push('\\');
    out.push_str(name);
    out.push(' ');
}
