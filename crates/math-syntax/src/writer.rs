//! Model tree back to LaTeX.
//!
//! The output normalizes whitespace and redundant braces, so a parsed document
//! round-trips structurally rather than byte for byte. A barless fraction is
//! written with `\atop`, the infix form that parses back without a rule.

use crate::colors;
use crate::spellings::Spellings;
use crate::symbols;
use math_core::{
    Alignment, Atom, AtomClass, ColumnSpacing, FractionRule, Length, LengthUnit, Limits, Model,
    NodeId, NodeKind, Origin, Size, Span, Style,
};

/// Where a node sits: a nested row in an item position needs braces to stay a
/// row, while an argument's braces are supplied by its caller.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Context {
    Top,
    Item,
    /// An item with a sibling after it: a declaration must close its scope.
    ItemBeforeSibling,
    Argument,
}

impl Context {
    fn item(self) -> bool {
        matches!(self, Context::Item | Context::ItemBeforeSibling)
    }
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

    fn environment(&self, id: NodeId) -> Option<&str> {
        self.spellings.environment(id)
    }

    fn spelling(&self, model: &Model, id: NodeId) -> Option<String> {
        let spelling = self.spellings.get(id)?;
        if spelling.matches(model, id) {
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
            if let Some(name) = out.environment(id).map(str::to_owned) {
                // An unwrapped `equation`: the model is just its formula.
                push_command(out, "begin");
                out.push('{');
                out.push_str(&name);
                out.push('}');
                write_row_children(model, children, out);
                push_command(out, "end");
                out.push('{');
                out.push_str(&name);
                out.push('}');
            } else {
                let braces = context.item();
                if braces {
                    out.push('{');
                }
                write_row_children(model, children, out);
                if braces {
                    out.push('}');
                }
            }
        }
        NodeKind::Scripts { base, sup, sub } => {
            // A declaration base keeps its body out of the script.
            write(model, *base, Context::ItemBeforeSibling, out);
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
            let environment = out.environment(id).map(str::to_owned);
            if let Some(name) = environment.as_deref()
                && let Some(table) = table_in_body(model, *body)
            {
                let argument = environment_argument(name, model, table);
                let (body_start, body_end) =
                    write_environment_with_arg(model, table, name, argument, out);
                // The fenced wrapper is spelled by the environment name, so
                // its body and table would otherwise lose their spans.
                out.spans.push((*body, Span::new(body_start, body_end)));
                out.spans.push((table, Span::new(body_start, body_end)));
            } else if let Some((name, table)) = cases_environment(model, *body, *left, *right) {
                write_environment(model, table, name, out);
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
            write_operator_limits(*symbol, *limits, out);
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
                // The spelling is used only when it parses to this placement
                // on its own; `\operatorname{lim}\displaylimits` must fall
                // back to `\operatorname*{lim}` instead.
                Some(text) if spelled_function_limits(&text, name) == Some(*limits) => {
                    out.push_token(&text);
                }
                _ => match symbols::function_command(name) {
                    // A named command can carry an explicit suffix, but not a
                    // different display default: only the intrinsic placement
                    // is spelled by the command itself.
                    Some((command, intrinsic))
                        if !matches!(*limits, Limits::Default { .. }) || *limits == intrinsic =>
                    {
                        push_command(out, command);
                        if *limits != intrinsic {
                            write_limits_suffix(*limits, out);
                        }
                    }
                    _ => {
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
                        write_limits_suffix(*limits, out);
                    }
                },
            }
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
        NodeKind::Phantom { body, kind } => match out.spelling(model, id) {
            Some(text) => out.push_token(&text),
            None => {
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
        },
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
        NodeKind::Styled { size, body } => {
            push_command(
                out,
                match size {
                    Size::Display => "displaystyle",
                    Size::Text => "textstyle",
                    Size::Script => "scriptstyle",
                    Size::ScriptScript => "scriptscriptstyle",
                },
            );
            write_declaration_body(model, *body, context, out);
        }
        NodeKind::ColorBox {
            background,
            border,
            body,
        } => {
            match border {
                Some(border) => {
                    push_command(out, "fcolorbox");
                    out.push('{');
                    out.push_str(&colors::write(*border));
                    out.push('}');
                }
                None => push_command(out, "colorbox"),
            }
            out.push('{');
            out.push_str(&colors::write(*background));
            out.push('}');
            // The body is a text argument, so `\text{…}` would be read back
            // as literal text; write the characters themselves.
            match model.kind(*body) {
                NodeKind::Text(text) => {
                    out.push('{');
                    write_text(text, out);
                    out.push('}');
                }
                _ => write_argument(model, *body, out),
            }
        }
        NodeKind::Color { color, body } => match out.spelling(model, id) {
            Some(text) => {
                out.push_token(&text);
                write_declaration_body(model, *body, context, out);
            }
            None => {
                push_command(out, "textcolor");
                out.push('{');
                out.push_str(&colors::write(*color));
                out.push('}');
                write_argument(model, *body, out);
            }
        },
        NodeKind::Class {
            class,
            body,
            limits,
        } => {
            match out.spelling(model, id) {
                Some(text) => out.push_token(&text),
                None => {
                    push_command(out, class_name(*class));
                    write_argument(model, *body, out);
                }
            }
            if *class == AtomClass::Op {
                write_limits_suffix(*limits, out);
            }
        }
        NodeKind::MathChoice {
            display,
            text,
            script,
            script_script,
        } => {
            push_command(out, "mathchoice");
            for choice in [display, text, script, script_script] {
                write_argument(model, *choice, out);
            }
        }
        NodeKind::UnderArrow { symbol, body } => {
            push_command(
                out,
                match symbol {
                    '\u{2190}' => "underleftarrow",
                    _ => "underrightarrow",
                },
            );
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
            columns,
            alignment,
            cell_style,
            column_rules,
            column_spacing,
            ..
        } => {
            let environment = out.environment(id).map(str::to_owned);
            if environment.as_deref() == Some("substack") {
                write_substack(model, id, out);
            } else {
                let (name, argument) = match environment.as_deref() {
                    Some(name) => (name, environment_argument(name, model, id)),
                    None => inferred_environment(
                        *columns,
                        alignment,
                        *cell_style,
                        column_rules,
                        *column_spacing,
                    ),
                };
                write_environment_with_arg(model, id, name, argument, out);
            }
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

/// The placement a function spelling parses to without a suffix.
fn spelled_function_limits(text: &str, name: &str) -> Option<Limits> {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix("\\operatorname") {
        return Some(Limits::Default {
            display_limits: rest.starts_with('*'),
        });
    }
    symbols::function_command(name).map(|(_, intrinsic)| intrinsic)
}

/// A large operator's limit switch. `\displaylimits` names the display default,
/// so it is written only when the operator's table default differs; writing it
/// for every display-limited operator would be noise.
fn write_operator_limits(symbol: char, limits: Limits, out: &mut Output<'_>) {
    let intrinsic = symbols::large_op_command(symbol)
        .and_then(symbols::large_op)
        .map(|(_, display_limits)| Limits::Default { display_limits });
    match limits {
        Limits::Default { display_limits } if intrinsic != Some(limits) => {
            push_command(
                out,
                if display_limits {
                    "displaylimits"
                } else {
                    "nolimits"
                },
            );
        }
        other => write_limits_suffix(other, out),
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

/// `\substack{…}` has table rows but is a command, not a fenced environment.
fn write_substack(model: &Model, table: NodeId, out: &mut Output<'_>) {
    let NodeKind::Table { rows, .. } = model.kind(table) else {
        return;
    };
    push_command(out, "substack");
    out.push('{');
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            out.push_str(" \\\\ ");
        }
        write(model, *row, Context::Argument, out);
    }
    out.push('}');
}

fn write_environment(model: &Model, table: NodeId, name: &str, out: &mut Output<'_>) {
    let _ = write_environment_with_arg(model, table, name, None, out);
}

/// The argument an environment takes after its name.
enum EnvironmentArgument {
    /// A starred matrix's `[l|c|r]`.
    Bracket(char),
    /// A braced argument written as spelled: `array`'s column specification or
    /// `alignat`'s pair count.
    Braces(String),
}

/// Writes `\begin{name}[arg] rows \end{name}` and returns the span of the
/// rows, which is what a fenced wrapper's body covers.
fn write_environment_with_arg(
    model: &Model,
    table: NodeId,
    name: &str,
    argument: Option<EnvironmentArgument>,
    out: &mut Output<'_>,
) -> (usize, usize) {
    let NodeKind::Table { rows, .. } = model.kind(table) else {
        let at = out.text.len();
        return (at, at);
    };
    push_command(out, "begin");
    out.push('{');
    out.push_str(name);
    out.push('}');
    match argument {
        Some(EnvironmentArgument::Bracket(letter)) => {
            out.push('[');
            out.push(letter);
            out.push(']');
        }
        Some(EnvironmentArgument::Braces(text)) => {
            out.push('{');
            out.push_str(&text);
            out.push('}');
        }
        None => {}
    }
    let body_start = out.text.len();
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            out.push_str(" \\\\ ");
        }
        write(model, *row, Context::Argument, out);
    }
    let body_end = out.text.len();
    push_command(out, "end");
    out.push('{');
    out.push_str(name);
    out.push('}');
    (body_start, body_end)
}

/// The table inside a fenced environment's body row.
fn table_in_body(model: &Model, body: NodeId) -> Option<NodeId> {
    let NodeKind::Row(content) = model.kind(body) else {
        return None;
    };
    let [table] = content.as_slice() else {
        return None;
    };
    matches!(model.kind(*table), NodeKind::Table { .. }).then_some(*table)
}

/// The argument an environment name implies, rebuilt from the model.
fn environment_argument(name: &str, model: &Model, table: NodeId) -> Option<EnvironmentArgument> {
    let NodeKind::Table {
        columns,
        alignment,
        column_rules,
        ..
    } = model.kind(table)
    else {
        return None;
    };
    match name {
        "array" | "subarray" => Some(EnvironmentArgument::Braces(column_preamble(
            *columns,
            alignment,
            column_rules,
        ))),
        "alignat" | "alignat*" | "alignedat" => Some(EnvironmentArgument::Braces(
            pair_count(*columns).to_string(),
        )),
        _ if name.ends_with('*') => {
            uniform_alignment(*columns, alignment).map(EnvironmentArgument::Bracket)
        }
        _ => None,
    }
}

/// The one alignment every column shares, when there is one.
fn uniform_alignment(columns: usize, alignment: &[Alignment]) -> Option<char> {
    if columns == 0 || alignment.len() != columns {
        return None;
    }
    let first = *alignment.first()?;
    if !alignment.iter().all(|align| *align == first) {
        return None;
    }
    Some(alignment_letter(first))
}

fn class_name(class: AtomClass) -> &'static str {
    match class {
        AtomClass::Ord => "mathord",
        AtomClass::Op => "mathop",
        AtomClass::Bin => "mathbin",
        AtomClass::Rel => "mathrel",
        AtomClass::Open => "mathopen",
        AtomClass::Close => "mathclose",
        AtomClass::Punct => "mathpunct",
        AtomClass::Inner => "mathinner",
    }
}

fn alignment_letter(alignment: Alignment) -> char {
    match alignment {
        Alignment::Left => 'l',
        Alignment::Center => 'c',
        Alignment::Right => 'r',
    }
}

/// How many `rl` column pairs a table with this many columns declares.
fn pair_count(columns: usize) -> usize {
    columns.div_ceil(2)
}

/// An `array` column specification rebuilt from the model: each boundary's
/// rules, then the next column's letter.
fn column_preamble(columns: usize, alignment: &[Alignment], column_rules: &[u8]) -> String {
    let rules = (column_rules.len() == columns + 1).then_some(column_rules);
    let mut preamble = String::new();
    for boundary in 0..=columns {
        let count = rules
            .and_then(|rules| rules.get(boundary))
            .copied()
            .unwrap_or(0);
        for _ in 0..count {
            preamble.push('|');
        }
        if boundary < columns {
            let align = alignment.get(boundary).copied().unwrap_or_default();
            preamble.push(alignment_letter(align));
        }
    }
    preamble
}

/// The environment a table's shape implies when no source name survives.
fn inferred_environment(
    columns: usize,
    alignment: &[Alignment],
    cell_style: Option<Style>,
    column_rules: &[u8],
    column_spacing: ColumnSpacing,
) -> (&'static str, Option<EnvironmentArgument>) {
    match cell_style.map(|style| style.size) {
        // A `smallmatrix` has no alignment, so a script table that carries one
        // stays a subarray.
        Some(Size::Script | Size::ScriptScript) if alignment.is_empty() => ("smallmatrix", None),
        Some(Size::Script | Size::ScriptScript) => (
            "subarray",
            Some(EnvironmentArgument::Braces(column_preamble(
                columns,
                alignment,
                column_rules,
            ))),
        ),
        Some(Size::Display) => {
            if !aligned_columns(columns, alignment) {
                ("gathered", None)
            } else if column_spacing == ColumnSpacing::Flush {
                let count = pair_count(columns).to_string();
                ("alignedat", Some(EnvironmentArgument::Braces(count)))
            } else {
                ("aligned", None)
            }
        }
        Some(Size::Text) => {
            if aligned_columns(columns, alignment) {
                ("aligned", None)
            } else if let Some(argument) = uniform_alignment(columns, alignment) {
                ("matrix*", Some(EnvironmentArgument::Bracket(argument)))
            } else {
                ("matrix", None)
            }
        }
        // `None` inherits the surrounding style, which only `array` spells.
        None => (
            "array",
            Some(EnvironmentArgument::Braces(column_preamble(
                columns,
                alignment,
                column_rules,
            ))),
        ),
    }
}

/// A fenced all-left table spells one of the cases environments; its cell
/// style selects `cases`/`rcases` (text) or `dcases`/`drcases` (display).
fn cases_environment(
    model: &Model,
    body: NodeId,
    left: Option<char>,
    right: Option<char>,
) -> Option<(&'static str, NodeId)> {
    let NodeKind::Row(content) = model.kind(body) else {
        return None;
    };
    let [table] = content.as_slice() else {
        return None;
    };
    let NodeKind::Table {
        alignment,
        cell_style,
        ..
    } = model.kind(*table)
    else {
        return None;
    };
    if alignment.is_empty() || !alignment.iter().all(|column| *column == Alignment::Left) {
        return None;
    }
    let size = cell_style.map_or(Size::Text, |style| style.size);
    let name = match (left, right, size) {
        (Some('{'), None, Size::Text) => "cases",
        (Some('{'), None, Size::Display) => "dcases",
        (None, Some('}'), Size::Text) => "rcases",
        (Some('{'), Some('}'), Size::Display) => "drcases",
        _ => return None,
    };
    Some((name, *table))
}

/// The parser unwraps a one-child braced argument; a second pair of braces
/// keeps a genuine one-item row from collapsing on the way back in.
///
/// A synthetic row is an editor's typing container, not a group the source
/// wrote, so its braces must not multiply.
fn write_row_children(model: &Model, children: &[NodeId], out: &mut Output<'_>) {
    for (index, child) in children.iter().enumerate() {
        let context = if index + 1 == children.len() {
            Context::Item
        } else {
            Context::ItemBeforeSibling
        };
        write(model, *child, context, out);
    }
}

/// A declaration's body is the rest of the group, so it needs braces only
/// when a sibling follows and would otherwise fall inside its scope.
fn write_declaration_body(model: &Model, body: NodeId, context: Context, out: &mut Output<'_>) {
    if context == Context::ItemBeforeSibling {
        out.push('{');
        write(model, body, Context::Argument, out);
        out.push('}');
    } else {
        write(model, body, Context::Argument, out);
    }
}

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
