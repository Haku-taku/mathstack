//! Recursive descent over the token stream. Every construct keeps its source
//! span, and every failure recovers in place so a later edit still has a tree
//! to work with.

use crate::lexer::{Token, TokenKind};
use crate::spellings::{Spelling, Spellings};
use crate::symbols;
use math_core::{
    Alignment, Atom, AtomClass, ColumnSpacing, Diagnostic, FractionRule, Length, LengthUnit,
    Limits, Model, ModelBuilder, NodeId, NodeKind, Origin, PhantomKind, Size, SpaceKind, Span,
    Style, Variant,
};

/// LaTeX nesting a formula can plausibly use; deeper input is skipped rather
/// than recursed into.
const MAX_DEPTH: usize = 64;
/// One huge row should not be able to exhaust memory.
const MAX_NODES: usize = 100_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stop {
    Top,
    Group,
    Left,
    Bracket,
    /// A grid cell: `&`, `\\`, `\end` and `}` end it.
    Cell,
    /// A single formula delimited only by `\end`; used by `equation`.
    Environment,
}

enum RowStep {
    Close,
    StrayBrace(Span),
    StrayRight(Span),
    Infix(String, Span),
    /// A style switch, which applies to the row's remaining children.
    Style(String),
    Item,
}

enum TextArgument {
    Text(String, Span),
    Unavailable(Span),
}

/// What the next token means to an argument position, decided before any
/// recursive call so the parser's token borrow ends.
enum Next {
    Group,
    Missing,
    Atom,
}

struct Parser<'a> {
    source: &'a str,
    model: ModelBuilder,
    tokens: Vec<Token>,
    index: usize,
    diagnostics: Vec<Diagnostic>,
    nodes: usize,
    over_budget: bool,
    /// The alphabet selected by the enclosing font command.
    variant: Variant,
    /// How many `\left` bodies enclose the current position.
    left_depth: usize,
    spellings: Spellings,
}

pub(crate) fn parse(
    source: &str,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
) -> (Model, Spellings, Vec<Diagnostic>) {
    let mut parser = Parser {
        source,
        model: ModelBuilder::new(),
        tokens,
        index: 0,
        diagnostics,
        nodes: 0,
        over_budget: false,
        variant: Variant::Normal,
        left_depth: 0,
        spellings: Spellings::default(),
    };
    let root = parser.parse_row(0, Stop::Top);
    (
        parser.model.build(root),
        parser.spellings,
        parser.diagnostics,
    )
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.index).cloned();
        if token.is_some() {
            self.index += 1;
        }
        token
    }

    /// The byte position where the next token starts, or the end of input.
    fn caret(&self) -> usize {
        self.peek()
            .map_or(self.source.len(), |token| token.span.start)
    }

    fn error(&mut self, span: Span, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::new(span, message));
    }

    fn make(&mut self, kind: NodeKind, span: Span) -> NodeId {
        self.nodes += 1;
        self.model.alloc(kind, span)
    }

    fn span_of(&self, id: NodeId) -> Span {
        self.model.span(id)
    }

    fn parse_row(&mut self, depth: usize, stop: Stop) -> NodeId {
        let start = self.caret();
        if depth > MAX_DEPTH {
            self.error(Span::empty(start), "formula nested too deeply");
            return self.skip_row(start, stop);
        }

        let mut children = Vec::new();
        // The style switch in effect and the row index where its scope starts.
        let mut pending: Option<(Size, Span, usize, Origin)> = None;
        loop {
            if self.nodes >= MAX_NODES {
                if !self.over_budget {
                    self.over_budget = true;
                    self.error(Span::empty(self.caret()), "formula too large");
                }
                break;
            }
            let Some(token) = self.peek() else { break };
            let step = match &token.kind {
                TokenKind::RBrace if stop == Stop::Group || stop == Stop::Cell => RowStep::Close,
                TokenKind::RBrace => RowStep::StrayBrace(token.span),
                TokenKind::Command(name) if name == "right" && stop == Stop::Left => RowStep::Close,
                TokenKind::Command(name) if name == "right" => RowStep::StrayRight(token.span),
                TokenKind::Symbol(']') if stop == Stop::Bracket => RowStep::Close,
                TokenKind::Symbol('&') if stop == Stop::Cell => RowStep::Close,
                TokenKind::Command(name) if name == "\\" && stop == Stop::Cell => RowStep::Close,
                TokenKind::Command(name)
                    if name == "end" && matches!(stop, Stop::Cell | Stop::Environment) =>
                {
                    RowStep::Close
                }
                TokenKind::Command(name) if is_infix(name) => {
                    RowStep::Infix(name.clone(), token.span)
                }
                TokenKind::Command(name) if style_size(name).is_some() => {
                    RowStep::Style(name.clone())
                }
                _ => RowStep::Item,
            };
            match step {
                RowStep::Close => break,
                RowStep::StrayBrace(span) => {
                    self.advance();
                    self.error(span, "unexpected `}`");
                }
                RowStep::StrayRight(span) => {
                    self.advance();
                    self.error(span, "`\\right` without `\\left`");
                }
                RowStep::Infix(name, command_span) => {
                    self.close_style(&mut children, &mut pending);
                    return self.parse_infix(&name, command_span, start, children, depth, stop);
                }
                RowStep::Style(name) => {
                    let token = self.advance().expect("peeked a command");
                    let size = style_size(&name).expect("matched above");
                    // The previous switch ends where this one starts.
                    self.close_style(&mut children, &mut pending);
                    pending = Some((size, token.span, children.len(), token.origin));
                }
                RowStep::Item => {
                    let item = self.parse_item(depth);
                    children.push(item);
                }
            }
        }
        self.close_style(&mut children, &mut pending);

        let end = children
            .last()
            .map_or_else(|| self.caret(), |child| self.span_of(*child).end);
        self.make(NodeKind::Row(children), Span::new(start, end.max(start)))
    }

    /// Wraps the siblings a style switch governs in its node. TeX scopes the
    /// switch to the rest of the group, so whatever follows belongs to it.
    fn close_style(
        &mut self,
        children: &mut Vec<NodeId>,
        pending: &mut Option<(Size, Span, usize, Origin)>,
    ) {
        let Some((size, command_span, first, origin)) = pending.take() else {
            return;
        };
        // A macro-expanded switch covers its call site, not the source that
        // follows the expansion.
        let expanded = origin != Origin::Source;
        let body_children = children.split_off(first);
        let end = if expanded {
            command_span.end
        } else {
            body_children
                .last()
                .map_or(command_span.end, |child| self.span_of(*child).end)
        };
        let body = self.make(
            NodeKind::Row(body_children),
            Span::new(command_span.end, end.max(command_span.end)),
        );
        let node = self.make(
            NodeKind::Styled { size, body },
            Span::new(command_span.start, end.max(command_span.end)),
        );
        if expanded {
            self.model.set_origin(node, origin);
        }
        children.push(node);
    }

    /// `\atop`, `\over` and the delimited variants consume the rest of the
    /// row as their denominator.
    fn parse_infix(
        &mut self,
        name: &str,
        command_span: Span,
        start: usize,
        children: Vec<NodeId>,
        depth: usize,
        stop: Stop,
    ) -> NodeId {
        self.advance();
        if children.is_empty() {
            self.error(command_span, format!("missing numerator before \\{name}"));
        }
        let numerator_end = children
            .last()
            .map_or(start, |child| self.span_of(*child).end);
        let numerator = self.make(
            NodeKind::Row(children),
            Span::new(start, numerator_end.max(start)),
        );
        let denominator = self.parse_row(depth, stop);
        if matches!(self.model.kind(denominator), NodeKind::Row(children) if children.is_empty()) {
            self.error(command_span, format!("missing denominator after \\{name}"));
        }
        let span = self
            .span_of(numerator)
            .join(self.span_of(denominator))
            .join(command_span);
        let rule = if name == "over" {
            FractionRule::Default
        } else {
            FractionRule::None
        };
        let fraction = self.make(
            NodeKind::Fraction {
                numerator,
                denominator,
                rule,
                style: None,
            },
            span,
        );
        let (left, right) = match name {
            "choose" => (Some('('), Some(')')),
            "brack" => (Some('['), Some(']')),
            "brace" => (Some('{'), Some('}')),
            _ => (None, None),
        };
        let node = match (left, right) {
            (None, None) => fraction,
            (left, right) => {
                let body = self.make(NodeKind::Row(vec![fraction]), span);
                self.make(NodeKind::Delimited { left, body, right }, span)
            }
        };
        self.make(NodeKind::Row(vec![node]), span)
    }

    /// Consumes tokens up to the row's terminator without recursing.
    fn skip_row(&mut self, start: usize, stop: Stop) -> NodeId {
        let mut braces = 0usize;
        let mut end = start;
        while let Some(token) = self.peek() {
            match &token.kind {
                TokenKind::RBrace if braces == 0 && stop == Stop::Group => break,
                TokenKind::RBrace if braces == 0 => {
                    end = token.span.end;
                    self.advance();
                }
                TokenKind::RBrace => {
                    braces -= 1;
                    end = token.span.end;
                    self.advance();
                }
                TokenKind::LBrace => {
                    braces += 1;
                    end = token.span.end;
                    self.advance();
                }
                TokenKind::Command(name)
                    if name == "right" && braces == 0 && stop == Stop::Left =>
                {
                    break;
                }
                TokenKind::Symbol(']') if braces == 0 && stop == Stop::Bracket => break,
                _ => {
                    end = token.span.end;
                    self.advance();
                }
            }
        }
        self.make(NodeKind::Row(Vec::new()), Span::new(start, end))
    }

    fn parse_item(&mut self, depth: usize) -> NodeId {
        // An item's outermost node inherits the origin of the token that
        // starts it; children keep their own, so macro arguments stay editable.
        let origin = self.peek().map_or(Origin::Source, |token| token.origin);
        let mut sup: Option<NodeId> = None;
        let mut sub: Option<NodeId> = None;

        let base = if matches!(
            self.peek().map(|token| &token.kind),
            Some(TokenKind::Sup | TokenKind::Sub)
        ) {
            let at = self.caret();
            self.error(Span::empty(at), "script without a base");
            self.make(NodeKind::Row(Vec::new()), Span::empty(at))
        } else {
            self.parse_atom(depth)
        };
        self.parse_limits_suffix(base);

        while let Some(is_sup) = self.next_script() {
            let marker = self.advance().expect("peeked a script marker");
            let argument = self.parse_script_argument(depth);
            let target = if is_sup { &mut sup } else { &mut sub };
            if target.is_some() {
                let what = if is_sup { "superscript" } else { "subscript" };
                self.error(marker.span, format!("double {what}"));
            } else {
                *target = Some(argument);
            }
        }

        let item = if sup.is_none() && sub.is_none() {
            base
        } else {
            let mut span = self.span_of(base);
            for script in sup.iter().chain(sub.iter()) {
                span = span.join(self.span_of(*script));
            }
            self.make(NodeKind::Scripts { base, sup, sub }, span)
        };
        if !matches!(origin, Origin::Source) {
            self.model.set_origin(item, origin);
        }
        item
    }

    /// `\limits` and `\nolimits` directly after an operator override its
    /// table default; TeX rejects them after a script or a non-operator.
    fn parse_limits_suffix(&mut self, base: NodeId) {
        loop {
            let name = match self.peek().map(|token| &token.kind) {
                Some(TokenKind::Command(name)) if name == "limits" || name == "nolimits" => {
                    name.clone()
                }
                _ => break,
            };
            let span = self.advance().expect("peeked a command").span;
            let limits = if name == "limits" {
                Limits::AboveBelow
            } else {
                Limits::Beside
            };
            let kind = match self.model.kind(base) {
                NodeKind::LargeOp { symbol, .. } => NodeKind::LargeOp {
                    symbol: *symbol,
                    limits,
                },
                NodeKind::Function { name, .. } => NodeKind::Function {
                    name: name.clone(),
                    limits,
                },
                // Only an explicit Op accepts the suffix; the other classes
                // are not operators.
                NodeKind::Class {
                    class: AtomClass::Op,
                    body,
                    ..
                } => NodeKind::Class {
                    class: AtomClass::Op,
                    body: *body,
                    limits,
                },
                _ => {
                    self.error(span, format!("`\\{name}` needs an operator"));
                    continue;
                }
            };
            self.model.replace(base, kind);
            if self.model.node(base).origin == Origin::Source {
                // The suffix is not part of the node's span, so its recorded
                // spelling stays valid for the new placement.
                let token = self.model.span(base);
                self.record_spelling(base, token, Origin::Source);
            }
        }
    }

    fn next_script(&self) -> Option<bool> {
        match self.peek().map(|token| &token.kind) {
            Some(TokenKind::Sup) => Some(true),
            Some(TokenKind::Sub) => Some(false),
            _ => None,
        }
    }

    fn parse_script_argument(&mut self, depth: usize) -> NodeId {
        let next = match self.peek().map(|token| &token.kind) {
            Some(TokenKind::LBrace) => Next::Group,
            Some(TokenKind::Command(name)) if name == "right" => Next::Missing,
            Some(TokenKind::RBrace | TokenKind::Sup | TokenKind::Sub) | None => Next::Missing,
            Some(_) => Next::Atom,
        };
        match next {
            Next::Group => {
                let group = self.parse_group(depth);
                self.unwrap_singleton(group)
            }
            Next::Missing => self.missing("script"),
            Next::Atom => self.parse_atom(depth),
        }
    }

    fn parse_atom(&mut self, depth: usize) -> NodeId {
        let Some(token) = self.peek().cloned() else {
            return self.missing("operand");
        };
        let atom = match token.kind {
            TokenKind::Symbol(symbol) => {
                self.advance();
                self.make(
                    NodeKind::Atom(Atom::with_variant(symbol, self.variant)),
                    token.span,
                )
            }
            TokenKind::Error(raw) => {
                self.advance();
                self.make(NodeKind::Error(raw), token.span)
            }
            TokenKind::LBrace => self.parse_group(depth),
            TokenKind::Command(name) => self.parse_command(&name, token.span, depth),
            _ => {
                self.advance();
                self.error(token.span, "unexpected token");
                let raw = self.source[token.span.start..token.span.end].to_owned();
                self.make(NodeKind::Error(raw), token.span)
            }
        };
        if !matches!(token.origin, Origin::Source) {
            self.model.set_origin(atom, token.origin);
        }
        self.record_spelling(atom, token.span, token.origin);
        atom
    }

    /// Keeps the token the document wrote when the writer would normalize it.
    ///
    /// Whole constructs use the node's span so a font command such as
    /// `\mathbb{R}` is stored with its argument; partial ones use the command
    /// token because their argument is written separately.
    fn record_spelling(&mut self, id: NodeId, token: Span, origin: Origin) {
        if origin != Origin::Source {
            return;
        }
        let kind = self.model.kind(id);
        let whole = matches!(
            kind,
            NodeKind::Atom(_)
                | NodeKind::LargeOp { .. }
                | NodeKind::Function { .. }
                | NodeKind::Text(_)
                | NodeKind::Space(_)
        );
        let span = if whole { self.model.span(id) } else { token };
        if !whole
            && !matches!(
                kind,
                NodeKind::Accent { .. } | NodeKind::SizedDelimiter { .. } | NodeKind::Middle { .. }
            )
        {
            return;
        }
        let Some(text) = self.source.get(span.start..span.end) else {
            return;
        };
        if let NodeKind::Atom(atom) = kind
            && atom.variant != Variant::Normal
            && !text.starts_with('\\')
        {
            // A bare symbol inside a font command must still be written with
            // that command, which only the canonical variant writer knows.
            return;
        }
        if let Some(spelling) = Spelling::from_kind(kind, text.to_owned()) {
            self.spellings.insert(id, spelling);
        }
    }

    fn parse_command(&mut self, name: &str, span: Span, depth: usize) -> NodeId {
        if depth > MAX_DEPTH {
            // Commands can nest without braces (`\sqrt\sqrt…`), so the row
            // depth alone does not bound the recursion.
            self.advance();
            self.error(span, "formula nested too deeply");
            return self.make(NodeKind::Row(Vec::new()), span);
        }
        match name {
            "begin" => {
                self.advance();
                self.parse_environment(span, depth)
            }
            "frac" | "dfrac" | "tfrac" | "cfrac" => {
                self.advance();
                let numerator = self.parse_argument(depth + 1, "\\frac");
                let denominator = self.parse_argument(depth + 1, "\\frac");
                let end = self
                    .span_of(denominator)
                    .end
                    .max(self.span_of(numerator).end);
                let style = match name {
                    "dfrac" | "cfrac" => Some(Size::Display),
                    "tfrac" => Some(Size::Text),
                    _ => None,
                };
                self.make(
                    NodeKind::Fraction {
                        numerator,
                        denominator,
                        rule: FractionRule::Default,
                        style,
                    },
                    Span::new(span.start, end),
                )
            }
            "xrightarrow" | "xleftarrow" => {
                self.advance();
                let mut end = span.end;
                let under = if matches!(
                    self.peek().map(|token| &token.kind),
                    Some(TokenKind::Symbol('['))
                ) {
                    self.advance();
                    let under = self.parse_row(depth + 1, Stop::Bracket);
                    if matches!(
                        self.peek().map(|token| &token.kind),
                        Some(TokenKind::Symbol(']'))
                    ) {
                        end = self.advance().map_or(end, |token| token.span.end);
                    } else {
                        let at = self.caret();
                        self.error(Span::new(span.start, at), "missing `]`");
                        end = at;
                    }
                    Some(under)
                } else {
                    None
                };
                let over = self.parse_argument(depth + 1, &format!("\\{name}"));
                end = end.max(self.span_of(over).end);
                let symbol = if name == "xrightarrow" {
                    '\u{2192}'
                } else {
                    '\u{2190}'
                };
                self.make(
                    NodeKind::Arrow {
                        symbol,
                        over: Some(over),
                        under,
                    },
                    Span::new(span.start, end),
                )
            }
            "overbrace" | "underbrace" => {
                self.advance();
                let body = self.parse_argument(depth + 1, &format!("\\{name}"));
                let end = self.span_of(body).end.max(span.end);
                self.make(
                    NodeKind::HorizontalBrace {
                        body,
                        over: name == "overbrace",
                    },
                    Span::new(span.start, end),
                )
            }
            "genfrac" => {
                self.advance();
                let left = self.genfrac_delimiter(span);
                let right = self.genfrac_delimiter(span);
                let rule = self.genfrac_rule(span);
                let style = self.genfrac_style(span);
                let mut numerator = self.parse_argument(depth + 1, "\\genfrac");
                let mut denominator = self.parse_argument(depth + 1, "\\genfrac");
                if matches!(rule, FractionRule::None) {
                    // The writer prints a ruleless fraction as the infix
                    // `\atop`, whose parts reparse as rows.
                    numerator = self.ensure_row(numerator);
                    denominator = self.ensure_row(denominator);
                }
                let end = self
                    .span_of(denominator)
                    .end
                    .max(self.span_of(numerator).end)
                    .max(span.end);
                let span = Span::new(span.start, end);
                let fraction = self.make(
                    NodeKind::Fraction {
                        numerator,
                        denominator,
                        rule,
                        style,
                    },
                    span,
                );
                match (left, right) {
                    (None, None) => fraction,
                    (left, right) => {
                        let body = self.make(NodeKind::Row(vec![fraction]), span);
                        self.make(NodeKind::Delimited { left, body, right }, span)
                    }
                }
            }
            "binom" | "dbinom" | "tbinom" => {
                // amsmath's binomials are `\left(…\right)` around a ruleless
                // fraction, so the existing nodes express them exactly.
                self.advance();
                let style = match name {
                    "dbinom" => Some(Size::Display),
                    "tbinom" => Some(Size::Text),
                    _ => None,
                };
                let command = format!("\\{name}");
                let numerator = self.parse_argument(depth + 1, &command);
                let denominator = self.parse_argument(depth + 1, &command);
                let end = self
                    .span_of(denominator)
                    .end
                    .max(self.span_of(numerator).end)
                    .max(span.end);
                let span = Span::new(span.start, end);
                let fraction = self.make(
                    NodeKind::Fraction {
                        numerator,
                        denominator,
                        rule: FractionRule::None,
                        style,
                    },
                    span,
                );
                let body = self.make(NodeKind::Row(vec![fraction]), span);
                self.make(
                    NodeKind::Delimited {
                        left: Some('('),
                        right: Some(')'),
                        body,
                    },
                    span,
                )
            }
            "not" => {
                self.advance();
                let argument = self.parse_atom(depth + 1);
                let end = self.span_of(argument).end.max(span.end);
                let span = Span::new(span.start, end);
                let target = match self.model.kind(argument) {
                    NodeKind::Atom(atom) => Some(*atom),
                    _ => None,
                };
                match target {
                    Some(atom) => match symbols::negate(atom.symbol) {
                        Some(symbol) => self.make(
                            NodeKind::Atom(Atom::with_variant(symbol, atom.variant)),
                            span,
                        ),
                        None => {
                            self.error(span, format!("cannot negate U+{:04X}", atom.symbol as u32));
                            argument
                        }
                    },
                    None => {
                        self.error(span, "`\\not` must precede a relation symbol");
                        argument
                    }
                }
            }
            "sqrt" => {
                self.advance();
                let mut end = span.end;
                let degree = if matches!(
                    self.peek().map(|token| &token.kind),
                    Some(TokenKind::Symbol('['))
                ) {
                    self.advance();
                    let degree = self.parse_row(depth + 1, Stop::Bracket);
                    if matches!(
                        self.peek().map(|token| &token.kind),
                        Some(TokenKind::Symbol(']'))
                    ) {
                        end = self.advance().expect("peeked `]`").span.end;
                    } else {
                        let at = self.caret();
                        self.error(Span::empty(at), "missing `]` after the index");
                    }
                    Some(degree)
                } else {
                    None
                };
                let body = self.parse_argument(depth + 1, "\\sqrt");
                end = end.max(self.span_of(body).end);
                self.make(
                    NodeKind::Radical { body, degree },
                    Span::new(span.start, end),
                )
            }
            "text" => {
                self.advance();
                self.parse_text(span)
            }
            "phantom" | "hphantom" | "vphantom" => {
                self.advance();
                let kind = match name {
                    "hphantom" => PhantomKind::Horizontal,
                    "vphantom" => PhantomKind::Vertical,
                    _ => PhantomKind::Full,
                };
                let body = self.parse_argument(depth + 1, &format!("\\{name}"));
                let end = self.span_of(body).end.max(span.end);
                self.make(NodeKind::Phantom { body, kind }, Span::new(span.start, end))
            }
            "smash" => {
                self.advance();
                let (mut top, mut bottom) = (true, true);
                if matches!(
                    self.peek().map(|token| &token.kind),
                    Some(TokenKind::Symbol('['))
                ) {
                    self.advance();
                    let mut option = String::new();
                    while let Some(token) = self.peek().cloned() {
                        match token.kind {
                            TokenKind::Symbol(']') => {
                                self.advance();
                                break;
                            }
                            TokenKind::Symbol(symbol) => {
                                option.push(symbol);
                                self.advance();
                            }
                            _ => {
                                self.error(token.span, "unsupported `\\smash` option");
                                self.advance();
                            }
                        }
                    }
                    match option.as_str() {
                        "t" => bottom = false,
                        "b" => top = false,
                        "" => {}
                        _ => self.error(span, format!("unknown `\\smash` option `{option}`")),
                    }
                }
                let body = self.parse_argument(depth + 1, "\\smash");
                let end = self.span_of(body).end.max(span.end);
                self.make(
                    NodeKind::Smash { body, top, bottom },
                    Span::new(span.start, end),
                )
            }
            "overline" | "underline" => {
                self.advance();
                let body = self.parse_argument(depth + 1, &format!("\\{name}"));
                let end = self.span_of(body).end.max(span.end);
                let kind = if name == "overline" {
                    NodeKind::Overline { body }
                } else {
                    NodeKind::Underline { body }
                };
                self.make(kind, Span::new(span.start, end))
            }
            "cancel" | "bcancel" | "xcancel" => {
                self.advance();
                let body = self.parse_argument(depth + 1, &format!("\\{name}"));
                let end = self.span_of(body).end.max(span.end);
                let (up, down) = match name {
                    "cancel" => (true, false),
                    "bcancel" => (false, true),
                    _ => (true, true),
                };
                self.make(
                    NodeKind::Cancel { body, up, down },
                    Span::new(span.start, end),
                )
            }
            "boxed" => {
                self.advance();
                let body = self.parse_argument(depth + 1, "\\boxed");
                let end = self.span_of(body).end.max(span.end);
                self.make(NodeKind::Boxed { body }, Span::new(span.start, end))
            }
            "mathord" | "mathop" | "mathbin" | "mathrel" | "mathopen" | "mathclose"
            | "mathpunct" | "mathinner" => {
                self.advance();
                let class = class_command(name).expect("matched above");
                let body = self.parse_argument(depth + 1, &format!("\\{name}"));
                let end = self.span_of(body).end.max(span.end);
                // `\mathop` alone takes limits in display style; the other
                // classes never do, so the flag only matters for it.
                let limits = Limits::Default {
                    display_limits: class == AtomClass::Op,
                };
                self.make(
                    NodeKind::Class {
                        class,
                        body,
                        limits,
                    },
                    Span::new(span.start, end),
                )
            }
            // A style switch reached outside a row (as a one-atom argument)
            // has no siblings to scope over.
            "displaystyle" | "textstyle" | "scriptstyle" | "scriptscriptstyle" => {
                self.advance();
                let size = style_size(name).expect("matched above");
                let body = self.make(NodeKind::Row(Vec::new()), Span::empty(span.end));
                self.make(NodeKind::Styled { size, body }, span)
            }
            "overset" | "underset" | "stackrel" => {
                self.advance();
                let annotation = self.parse_argument(depth + 1, &format!("\\{name}"));
                let base = self.parse_argument(depth + 1, &format!("\\{name}"));
                let end = self.span_of(base).end.max(self.span_of(annotation).end);
                let (over, under) = if name == "overset" || name == "stackrel" {
                    (Some(annotation), None)
                } else {
                    (None, Some(annotation))
                };
                self.make(
                    NodeKind::OverUnder { base, over, under },
                    Span::new(span.start, end),
                )
            }
            "pmod" => {
                self.advance();
                let argument = self.parse_argument(depth + 1, "\\pmod");
                let end = self.span_of(argument).end.max(span.end);
                let span = Span::new(span.start, end);
                // amsmath sets a quad, then "(mod", a thin space and the argument.
                let open = self.make(NodeKind::Text("(mod".to_owned()), span);
                let gap = self.make(NodeKind::Space(SpaceKind::Thin), span);
                let close = self.make(NodeKind::Text(")".to_owned()), span);
                let space = self.make(NodeKind::Space(SpaceKind::Quad), span);
                self.make(NodeKind::Row(vec![space, open, gap, argument, close]), span)
            }
            "pod" => {
                self.advance();
                let argument = self.parse_argument(depth + 1, "\\pod");
                let end = self.span_of(argument).end.max(span.end);
                let span = Span::new(span.start, end);
                // amsmath sets a quad, then the argument in parentheses.
                let open = self.make(NodeKind::Text("(".to_owned()), span);
                let close = self.make(NodeKind::Text(")".to_owned()), span);
                let space = self.make(NodeKind::Space(SpaceKind::Quad), span);
                self.make(NodeKind::Row(vec![space, open, argument, close]), span)
            }
            "mod" => {
                self.advance();
                let argument = self.parse_argument(depth + 1, "\\mod");
                let end = self.span_of(argument).end.max(span.end);
                let span = Span::new(span.start, end);
                // amsmath sets a quad, "mod", a thin space and the argument.
                let space = self.make(NodeKind::Space(SpaceKind::Quad), span);
                let text = self.make(NodeKind::Text("mod".to_owned()), span);
                let gap = self.make(NodeKind::Space(SpaceKind::Thin), span);
                self.make(NodeKind::Row(vec![space, text, gap, argument]), span)
            }
            "bmod" => {
                self.advance();
                let left = self.make(NodeKind::Space(SpaceKind::Medium), span);
                let text = self.make(NodeKind::Text("mod".to_owned()), span);
                let right = self.make(NodeKind::Space(SpaceKind::Medium), span);
                self.make(NodeKind::Row(vec![left, text, right]), span)
            }
            "hspace" => {
                self.advance();
                match self.text_argument("\\hspace", span) {
                    TextArgument::Text(text, text_span) => match parse_length(&text) {
                        Some(length) => self.make(NodeKind::Hspace(length), text_span),
                        None => {
                            self.error(text_span, format!("unsupported length `{text}`"));
                            self.make(NodeKind::Error("\\hspace".to_owned()), text_span)
                        }
                    },
                    TextArgument::Unavailable(text_span) => {
                        self.make(NodeKind::Error("\\hspace".to_owned()), text_span)
                    }
                }
            }
            "substack" => {
                self.advance();
                self.parse_substack(span, depth)
            }
            "operatorname" | "operatornamewithlimits" => {
                self.advance();
                let limits = name == "operatornamewithlimits"
                    || matches!(
                        self.peek().map(|token| &token.kind),
                        Some(TokenKind::Symbol('*'))
                    );
                if limits && name == "operatorname" {
                    self.advance();
                }
                let command = format!("\\{name}");
                match self.text_argument(&command, span) {
                    TextArgument::Text(name, span) => self.make(
                        NodeKind::Function {
                            name,
                            limits: limits.into(),
                        },
                        span,
                    ),
                    TextArgument::Unavailable(span) => self.make(NodeKind::Error(command), span),
                }
            }
            "left" => {
                self.advance();
                let left = self.parse_delimiter();
                self.left_depth += 1;
                let body = self.parse_row(depth + 1, Stop::Left);
                self.left_depth -= 1;
                let mut end = self.span_of(body).end;
                let right = if matches!(
                    self.peek().map(|token| &token.kind),
                    Some(TokenKind::Command(name)) if name == "right"
                ) {
                    self.advance();
                    let right = self.parse_delimiter();
                    if let Some(previous) =
                        self.index.checked_sub(1).and_then(|i| self.tokens.get(i))
                    {
                        end = previous.span.end.max(end);
                    }
                    right
                } else {
                    let at = self.caret();
                    self.error(Span::empty(at), "`\\left` without `\\right`");
                    None
                };
                self.make(
                    NodeKind::Delimited { left, body, right },
                    Span::new(span.start, end),
                )
            }
            _ => {
                self.advance();
                if let Some(variant) = symbols::font_command(name) {
                    let previous = self.variant;
                    self.variant = variant;
                    let argument = self.parse_argument(depth + 1, &format!("\\{name}"));
                    self.variant = previous;
                    let end = self.span_of(argument).end.max(span.end);
                    self.model.set_span(argument, Span::new(span.start, end));
                    argument
                } else if let Some(symbol) = symbols::accent(name) {
                    let base = self.parse_argument(depth + 1, &format!("\\{name}"));
                    let end = self.span_of(base).end.max(span.end);
                    self.make(
                        NodeKind::Accent { base, symbol },
                        Span::new(span.start, end),
                    )
                } else if let Some(kind) = symbols::space(name) {
                    self.make(NodeKind::Space(kind), span)
                } else if let Some((size, class)) = symbols::sized_delimiter(name) {
                    let symbol = self.parse_delimiter();
                    let end = self.delimiter_end(span);
                    match symbol {
                        Some(symbol) => self.make(
                            NodeKind::SizedDelimiter {
                                symbol,
                                size,
                                class,
                            },
                            Span::new(span.start, end),
                        ),
                        // `\big.` is TeX's invisible fence.
                        None => self.make(NodeKind::Row(Vec::new()), Span::new(span.start, end)),
                    }
                } else if name == "middle" {
                    let symbol = self.parse_delimiter();
                    let end = self.delimiter_end(span);
                    if self.left_depth == 0 {
                        self.error(span, "`\\middle` without `\\left`");
                    }
                    match symbol {
                        Some(symbol) => {
                            self.make(NodeKind::Middle { symbol }, Span::new(span.start, end))
                        }
                        None => self.make(NodeKind::Row(Vec::new()), Span::new(span.start, end)),
                    }
                } else if let Some((display, limits)) = symbols::function(name) {
                    self.make(
                        NodeKind::Function {
                            name: display.to_owned(),
                            limits,
                        },
                        span,
                    )
                } else if let Some((symbol, limits)) = symbols::large_op(name) {
                    self.make(
                        NodeKind::LargeOp {
                            symbol,
                            limits: limits.into(),
                        },
                        span,
                    )
                } else if let Some(symbol) = symbols::command_char(name) {
                    self.make(
                        NodeKind::Atom(Atom::with_variant(symbol, self.variant)),
                        span,
                    )
                } else {
                    let raw = format!("\\{name}");
                    self.error(span, format!("unknown command {raw}"));
                    self.make(NodeKind::Error(raw), span)
                }
            }
        }
    }

    /// `\begin{env} … \end{env}` for the table family and `equation`.
    fn parse_environment(&mut self, span: Span, depth: usize) -> NodeId {
        let (name, name_span) = match self.text_argument("\\begin", span) {
            TextArgument::Text(name, span) => (name.trim().to_owned(), span),
            TextArgument::Unavailable(span) => {
                return self.make(NodeKind::Error("\\begin".to_owned()), span);
            }
        };
        let Some(environment) = Environment::lookup(&name) else {
            self.error(name_span, format!("unsupported environment `{name}`"));
            return self.make(NodeKind::Error(format!("\\begin{{{name}}}")), name_span);
        };

        if environment.formula {
            let content = self.parse_row(depth + 1, Stop::Environment);
            let end_span = if self.at_environment_end() {
                self.take_environment_end(&name, name_span)
            } else {
                let at = self.caret();
                self.error(
                    Span::new(span.start, at),
                    format!("missing `\\end{{{name}}}`"),
                );
                Span::empty(at)
            };
            let content_span = Span::new(span.start, end_span.end.max(self.span_of(content).end));
            self.model.set_span(content, content_span);
            self.spellings.set_environment(content, name);
            return content;
        }

        let mut column_alignment = None;
        let mut column_spec = None;
        let mut pair_count = None;
        match environment.argument {
            EnvironmentArgument::None => {}
            EnvironmentArgument::OptionalAlignment => {
                column_alignment = self.parse_column_alignment();
            }
            EnvironmentArgument::Columns => {
                column_spec = Some(self.parse_column_spec(&name, span));
            }
            EnvironmentArgument::Pairs => pair_count = Some(self.parse_pair_count(&name, span)),
        }
        let declared = column_spec
            .as_ref()
            .map(|spec| spec.alignment.len())
            .or_else(|| pair_count.map(|pairs| pairs * 2));
        let mut cells: Vec<Vec<NodeId>> = Vec::new();
        let mut columns = 0;
        let mut overflow = None;
        let end_span;
        loop {
            if self.at_environment_end() {
                end_span = self.take_environment_end(&name, name_span);
                break;
            }
            if self.peek().is_none() {
                let at = self.caret();
                self.error(
                    Span::new(span.start, at),
                    format!("missing `\\end{{{name}}}`"),
                );
                end_span = Span::empty(at);
                break;
            }
            let mut row = Vec::new();
            loop {
                let cell = self.parse_row(depth + 1, Stop::Cell);
                row.push(cell);
                if matches!(
                    self.peek().map(|token| &token.kind),
                    Some(TokenKind::Symbol('&'))
                ) {
                    self.advance();
                    continue;
                }
                break;
            }
            if let Some(declared) = declared
                && declared > 0
                && row.len() > declared
            {
                overflow.get_or_insert(self.span_of(row[declared]));
            }
            let empty_row = row.len() == 1
                && matches!(self.model.kind(row[0]), NodeKind::Row(children) if children.is_empty());
            columns = columns.max(row.len());
            // A trailing `\\` before `\end` adds no row of its own.
            if !empty_row {
                cells.push(row);
            }
            if self.at_environment_end() {
                end_span = self.take_environment_end(&name, name_span);
                break;
            }
            if matches!(
                self.peek().map(|token| &token.kind),
                Some(TokenKind::Command(command)) if command == "\\"
            ) {
                self.advance();
            } else if self.peek().is_none() {
                continue;
            } else {
                let token = self.advance();
                if let Some(token) = token {
                    self.error(token.span, format!("expected `\\` or `\\end{{{name}}}`"));
                }
                continue;
            }
        }
        if let Some(span) = overflow {
            self.error(span, format!("more cells than `{name}` declares"));
        }

        columns = columns.max(declared.unwrap_or(0)).max(1);
        let mut alignment = match environment.alignment {
            EnvironmentAlignment::Cases => vec![Alignment::Left; columns],
            // `aligned`, `split` and `alignat` alternate a right column and a
            // left one.
            EnvironmentAlignment::Aligned => (0..columns)
                .map(|column| {
                    if column % 2 == 0 {
                        Alignment::Right
                    } else {
                        Alignment::Left
                    }
                })
                .collect(),
            // The table layout centers a column with no explicit alignment.
            EnvironmentAlignment::Center => Vec::new(),
            EnvironmentAlignment::Columns => {
                column_alignment.map_or_else(Vec::new, |alignment| vec![alignment; columns])
            }
            EnvironmentAlignment::Specified => column_spec
                .as_ref()
                .map_or_else(Vec::new, |spec| spec.alignment.clone()),
        };
        if matches!(environment.alignment, EnvironmentAlignment::Specified) {
            // A row wider than the declared spec keeps its content; the extra
            // columns fall back to the default alignment.
            alignment.resize(columns, Alignment::Center);
        }
        let column_rules = column_spec
            .map(|mut spec| {
                spec.rules.resize(columns + 1, 0);
                spec.rules
            })
            .filter(|rules| rules.iter().any(|count| *count > 0))
            .unwrap_or_default();
        let table_span = Span::new(span.start, end_span.end.max(span.end));
        let table = self.build_table(
            cells,
            TableShape {
                columns,
                alignment,
                cell_style: environment.cell_style,
                column_rules,
                column_spacing: environment.column_spacing,
            },
            table_span,
            end_span.start,
        );
        let node = match environment.delimiters {
            None => table,
            Some((left, right)) => {
                let body = self.make(NodeKind::Row(vec![table]), table_span);
                self.make(NodeKind::Delimited { left, body, right }, table_span)
            }
        };
        self.spellings.set_environment(node, name);
        node
    }

    /// The `[l|c|r]` argument of a starred matrix, which applies to every
    /// column. None means the environment's centered default.
    fn parse_column_alignment(&mut self) -> Option<Alignment> {
        let next = self.peek().and_then(|token| match &token.kind {
            TokenKind::Symbol(symbol) => Some(*symbol),
            _ => None,
        });
        if next != Some('[') {
            return None;
        }
        self.advance();
        let alignment = match self.peek().and_then(|token| match &token.kind {
            TokenKind::Symbol(symbol) => Some(*symbol),
            _ => None,
        }) {
            Some('l') => {
                self.advance();
                Some(Alignment::Left)
            }
            Some('c') => {
                self.advance();
                Some(Alignment::Center)
            }
            Some('r') => {
                self.advance();
                Some(Alignment::Right)
            }
            _ => {
                let next = self.peek().map(|token| {
                    (
                        token.span,
                        matches!(&token.kind, TokenKind::Command(name) if name == "end"),
                    )
                });
                match next {
                    Some((span, false)) => {
                        self.advance();
                        self.error(span, "expected `l`, `c` or `r`");
                    }
                    Some((span, true)) => self.error(span, "expected `l`, `c` or `r`"),
                    None => self.error(Span::empty(self.caret()), "expected `l`, `c` or `r`"),
                }
                None
            }
        };
        if matches!(
            self.peek().map(|token| &token.kind),
            Some(TokenKind::Symbol(']'))
        ) {
            self.advance();
        } else {
            self.error(Span::empty(self.caret()), "expected `]`");
            // Recover to the closing bracket so the rows still parse.
            while let Some(token) = self.peek() {
                match &token.kind {
                    TokenKind::Symbol(']') => {
                        self.advance();
                        break;
                    }
                    TokenKind::Command(name) if name == "end" => break,
                    _ => {
                        self.advance();
                    }
                }
            }
        }
        alignment
    }

    /// The `{l|c|r}` argument of `array`: one alignment per column and the
    /// vertical rules at the boundaries between them.
    fn parse_column_spec(&mut self, name: &str, span: Span) -> ColumnSpec {
        let mut spec = ColumnSpec {
            alignment: Vec::new(),
            rules: vec![0],
        };
        let command = format!("\\begin{{{name}}}");
        let TextArgument::Text(text, argument_span) = self.text_argument(&command, span) else {
            return spec;
        };
        let mut reported = false;
        for symbol in text.chars() {
            match symbol {
                'l' | 'c' | 'r' => {
                    spec.alignment.push(match symbol {
                        'l' => Alignment::Left,
                        'r' => Alignment::Right,
                        _ => Alignment::Center,
                    });
                    spec.rules.push(0);
                }
                '|' => {
                    if let Some(boundary) = spec.rules.last_mut() {
                        *boundary += 1;
                    }
                }
                symbol if symbol.is_whitespace() => {}
                _ => {
                    if !reported {
                        self.error(
                            argument_span,
                            format!("unsupported column specifier `{symbol}`"),
                        );
                        reported = true;
                    }
                }
            }
        }
        if spec.alignment.is_empty() {
            self.error(
                argument_span,
                "expected `l`, `c` or `r` in the column specification",
            );
        }
        spec
    }

    /// `alignat`'s mandatory `{n}`: how many right/left column pairs exist.
    fn parse_pair_count(&mut self, name: &str, span: Span) -> usize {
        let command = format!("\\begin{{{name}}}");
        let TextArgument::Text(text, argument_span) = self.text_argument(&command, span) else {
            return 0;
        };
        match text.trim().parse::<usize>() {
            Ok(count) if count > 0 => count,
            _ => {
                self.error(argument_span, "expected a positive column-pair count");
                0
            }
        }
    }

    /// Pads rows to the shape's column count and wraps them in a table. `tail`
    /// is where the padding cells point when nothing was typed.
    fn build_table(
        &mut self,
        cells: Vec<Vec<NodeId>>,
        shape: TableShape,
        table_span: Span,
        tail: usize,
    ) -> NodeId {
        let TableShape {
            columns,
            alignment,
            cell_style,
            column_rules,
            column_spacing,
        } = shape;
        let pad_span = Span::empty(tail);
        let mut rows = Vec::with_capacity(cells.len());
        for mut row in cells {
            while row.len() < columns {
                row.push(self.make(NodeKind::Row(Vec::new()), pad_span));
            }
            let end = row.last().map_or(tail, |cell| self.span_of(*cell).end);
            let row_span = Span::new(self.span_of(row[0]).start, end);
            rows.push(self.make(NodeKind::TableRow { cells: row }, row_span));
        }
        self.make(
            NodeKind::Table {
                rows,
                columns,
                alignment,
                cell_style,
                column_rules,
                column_spacing,
            },
            table_span,
        )
    }

    /// `\substack{…}`: a grid held in one script position, so rows are
    /// separated by `\\` and the closing brace ends the grid.
    fn parse_substack(&mut self, span: Span, depth: usize) -> NodeId {
        let Some(open) = self.peek().cloned() else {
            self.error(span, "missing argument for \\substack");
            return self.make(NodeKind::Error("\\substack".to_owned()), span);
        };
        if !matches!(open.kind, TokenKind::LBrace) {
            self.error(open.span, "expected `{` after \\substack");
            return self.make(NodeKind::Error("\\substack".to_owned()), span);
        }
        self.advance();

        let mut cells: Vec<Vec<NodeId>> = Vec::new();
        let mut columns = 0;
        let end = loop {
            if matches!(
                self.peek().map(|token| &token.kind),
                Some(TokenKind::RBrace)
            ) {
                break self.advance().map_or(span.end, |token| token.span.end);
            }
            if self.peek().is_none() {
                let at = self.caret();
                self.error(Span::new(span.start, at), "missing `}` after \\substack");
                break at;
            }
            let mut row = Vec::new();
            loop {
                let cell = self.parse_row(depth + 1, Stop::Cell);
                row.push(cell);
                if matches!(
                    self.peek().map(|token| &token.kind),
                    Some(TokenKind::Symbol('&'))
                ) {
                    self.advance();
                    continue;
                }
                break;
            }
            let empty_row = row.len() == 1
                && matches!(self.model.kind(row[0]), NodeKind::Row(children) if children.is_empty());
            columns = columns.max(row.len());
            if !empty_row {
                cells.push(row);
            }
            if matches!(
                self.peek().map(|token| &token.kind),
                Some(TokenKind::RBrace)
            ) {
                break self.advance().map_or(span.end, |token| token.span.end);
            }
            if matches!(
                self.peek().map(|token| &token.kind),
                Some(TokenKind::Command(command)) if command == "\\"
            ) {
                self.advance();
            } else {
                let token = self.advance();
                if let Some(token) = token {
                    self.error(token.span, "expected `\\` or `}`");
                }
            }
        };

        let columns = columns.max(1);
        let table_span = Span::new(span.start, end.max(span.end));
        let table = self.build_table(
            cells,
            TableShape {
                columns,
                alignment: Vec::new(),
                cell_style: Some(Style::SCRIPT),
                column_rules: Vec::new(),
                column_spacing: ColumnSpacing::Spaced,
            },
            table_span,
            end,
        );
        self.spellings.set_environment(table, "substack".to_owned());
        table
    }

    fn at_environment_end(&self) -> bool {
        matches!(
            self.peek().map(|token| &token.kind),
            Some(TokenKind::Command(name)) if name == "end"
        )
    }

    fn take_environment_end(&mut self, expected: &str, fallback: Span) -> Span {
        self.advance();
        match self.text_argument("\\end", fallback) {
            TextArgument::Text(name, span) => {
                if name.trim() != expected {
                    self.error(span, format!("expected `\\end{{{expected}}}`"));
                }
                span
            }
            TextArgument::Unavailable(span) => span,
        }
    }

    /// Text mode keeps the raw source between the braces, so spaces and
    /// nested groups survive; commands inside are literal for now.
    fn parse_text(&mut self, span: Span) -> NodeId {
        match self.text_argument("\\text", span) {
            TextArgument::Text(text, span) => {
                let decoded = decode_text(&text);
                self.make(NodeKind::Text(decoded), span)
            }
            TextArgument::Unavailable(span) => {
                self.make(NodeKind::Error("\\text".to_owned()), span)
            }
        }
    }

    /// One of `\genfrac`'s delimiter arguments, read as source text.
    fn genfrac_delimiter(&mut self, command_span: Span) -> Option<char> {
        match self.text_argument("\\genfrac", command_span) {
            TextArgument::Text(text, span) => match parse_delimiter_text(&text) {
                Some(delimiter) => delimiter,
                None => {
                    self.error(span, format!("unsupported delimiter `{text}`"));
                    None
                }
            },
            TextArgument::Unavailable(_) => None,
        }
    }

    fn ensure_row(&mut self, id: NodeId) -> NodeId {
        if matches!(self.model.kind(id), NodeKind::Row(_)) {
            return id;
        }
        let span = self.span_of(id);
        self.make(NodeKind::Row(vec![id]), span)
    }

    fn genfrac_rule(&mut self, command_span: Span) -> FractionRule {
        match self.text_argument("\\genfrac", command_span) {
            TextArgument::Text(text, span) => {
                let text = text.trim();
                if text.is_empty() {
                    return FractionRule::Default;
                }
                match parse_length_or_zero(text) {
                    Some(length) if length.milli == 0 => FractionRule::None,
                    Some(length) => FractionRule::Custom(length),
                    None => {
                        self.error(span, format!("unsupported thickness `{text}`"));
                        FractionRule::Default
                    }
                }
            }
            TextArgument::Unavailable(_) => FractionRule::Default,
        }
    }

    fn genfrac_style(&mut self, command_span: Span) -> Option<Size> {
        match self.text_argument("\\genfrac", command_span) {
            TextArgument::Text(text, span) => {
                let text = text.trim();
                if text.is_empty() {
                    return None;
                }
                match text {
                    "0" => Some(Size::Display),
                    "1" => Some(Size::Text),
                    "2" => Some(Size::Script),
                    "3" => Some(Size::ScriptScript),
                    _ => {
                        self.error(span, format!("invalid style `{text}`"));
                        None
                    }
                }
            }
            TextArgument::Unavailable(_) => None,
        }
    }

    fn text_argument(&mut self, command: &str, span: Span) -> TextArgument {
        let Some(open) = self.peek().cloned() else {
            self.error(span, format!("missing argument for {command}"));
            return TextArgument::Unavailable(span);
        };
        if !matches!(open.kind, TokenKind::LBrace) {
            self.error(open.span, format!("expected `{{` after {command}"));
            return TextArgument::Unavailable(Span::empty(open.span.start));
        }
        self.advance();
        let mut depth = 0;
        let mut close = None;
        for index in self.index..self.tokens.len() {
            match &self.tokens[index].kind {
                TokenKind::RBrace if depth == 0 => {
                    close = Some(index);
                    break;
                }
                TokenKind::RBrace => depth -= 1,
                TokenKind::LBrace => depth += 1,
                _ => {}
            }
        }
        match close {
            Some(index) => {
                let close_span = self.tokens[index].span;
                self.index = index + 1;
                let span = Span::new(span.start, close_span.end);
                // A macro body's tokens all carry the call span, so the bytes
                // between the braces are not the text the body spelled.
                if open.span.end > close_span.start {
                    self.error(
                        span,
                        "text mode inside an expanded macro cannot recover its source",
                    );
                    return TextArgument::Unavailable(span);
                }
                TextArgument::Text(
                    self.source[open.span.end..close_span.start].to_owned(),
                    span,
                )
            }
            None => {
                let text = self.source[open.span.end..].to_owned();
                let end = self.source.len();
                self.index = self.tokens.len();
                self.error(Span::new(span.start, end), "unclosed text argument");
                TextArgument::Text(text, Span::new(span.start, end))
            }
        }
    }

    fn parse_group(&mut self, depth: usize) -> NodeId {
        let open = self.advance().expect("caller saw `{`");
        let row = self.parse_row(depth + 1, Stop::Group);
        let span = if matches!(
            self.peek().map(|token| &token.kind),
            Some(TokenKind::RBrace)
        ) {
            let close = self.advance().expect("peeked `}`");
            Span::new(open.span.start, close.span.end)
        } else {
            let end = self.span_of(row).end.max(open.span.end);
            self.error(Span::new(open.span.start, end), "unclosed `{`");
            Span::new(open.span.start, end)
        };
        self.model.set_span(row, span);
        row
    }

    fn parse_argument(&mut self, depth: usize, command: &str) -> NodeId {
        let next = match self.peek().map(|token| &token.kind) {
            Some(TokenKind::LBrace) => Next::Group,
            Some(TokenKind::Command(name)) if name == "right" => Next::Missing,
            Some(TokenKind::RBrace | TokenKind::Sup | TokenKind::Sub) | None => Next::Missing,
            Some(_) => Next::Atom,
        };
        match next {
            Next::Group => {
                let group = self.parse_group(depth);
                self.unwrap_singleton(group)
            }
            Next::Missing => self.missing(command),
            Next::Atom => self.parse_atom(depth),
        }
    }

    fn missing(&mut self, what: &str) -> NodeId {
        let at = self.caret();
        self.error(Span::empty(at), format!("missing argument for {what}"));
        self.make(NodeKind::Row(Vec::new()), Span::empty(at))
    }

    /// A braced argument's braces group it but do not add a row: `x^{2}` and
    /// `x^2` must produce the same argument node.
    fn unwrap_singleton(&mut self, node: NodeId) -> NodeId {
        let span = self.span_of(node);
        let child = match self.model.kind(node) {
            NodeKind::Row(children) if children.len() == 1 => Some(children[0]),
            _ => None,
        };
        match child {
            Some(child) => {
                // The argument's extent includes its braces, so a construct's
                // span reaches its last closing brace.
                self.model.set_span(child, span);
                child
            }
            None => node,
        }
    }

    fn delimiter_end(&self, fallback: Span) -> usize {
        self.index
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .map_or(fallback.end, |token| token.span.end.max(fallback.end))
    }

    fn parse_delimiter(&mut self) -> Option<char> {
        let Some(token) = self.peek().cloned() else {
            let at = self.caret();
            self.error(Span::empty(at), "missing delimiter");
            return None;
        };
        match token.kind {
            TokenKind::Symbol('.') => {
                self.advance();
                None
            }
            TokenKind::Symbol(symbol) => {
                self.advance();
                Some(symbol)
            }
            TokenKind::Command(name) => {
                self.advance();
                if let Some(symbol) = symbols::delimiter(&name) {
                    Some(symbol)
                } else {
                    self.error(token.span, format!("unknown delimiter \\{name}"));
                    None
                }
            }
            _ => {
                self.error(token.span, "missing delimiter");
                None
            }
        }
    }
}

/// One accepted environment: what its body is and how its columns align.
#[derive(Clone, Copy)]
struct Environment {
    delimiters: Option<(Option<char>, Option<char>)>,
    alignment: EnvironmentAlignment,
    argument: EnvironmentArgument,
    /// `equation` holds one formula instead of a grid.
    formula: bool,
    /// The style the environment sets its cells in, as KaTeX's `dCellStyle`
    /// and the array/matrix definitions do; `None` inherits the table's, as
    /// `array` does.
    cell_style: Option<Style>,
    column_spacing: ColumnSpacing,
}

/// The argument an environment takes after its name.
#[derive(Clone, Copy)]
enum EnvironmentArgument {
    None,
    /// `matrix*`'s optional `[l|c|r]`, which applies to every column.
    OptionalAlignment,
    /// `array`'s mandatory `{l|c|r}` column specification.
    Columns,
    /// `alignat`'s mandatory `{n}` column-pair count.
    Pairs,
}

#[derive(Clone, Copy)]
enum EnvironmentAlignment {
    /// A matrix: centered unless the starred form names a column alignment.
    Columns,
    /// One alignment per column, from the environment's column specification.
    Specified,
    /// Both columns left-aligned (`cases` and its variants).
    Cases,
    /// A right column followed by a left one (`aligned`, `split`, `alignat`).
    Aligned,
    /// One centered column (`gather`, `gathered`).
    Center,
}

/// The alignment and boundary rules an `array` preamble spells.
struct ColumnSpec {
    alignment: Vec<Alignment>,
    /// One count per boundary, left to right; one more entry than `alignment`.
    rules: Vec<u8>,
}

/// The table shape an environment's arguments and rows settle on.
struct TableShape {
    columns: usize,
    alignment: Vec<Alignment>,
    cell_style: Option<Style>,
    column_rules: Vec<u8>,
    column_spacing: ColumnSpacing,
}

impl Environment {
    fn lookup(name: &str) -> Option<Self> {
        let grid = |delimiters, alignment, argument, cell_style, column_spacing| Environment {
            delimiters,
            alignment,
            argument,
            formula: false,
            cell_style,
            column_spacing,
        };
        // amsmath sets matrix and cases cells in text style.
        let matrix = |delimiters, argument| {
            grid(
                delimiters,
                EnvironmentAlignment::Columns,
                argument,
                Some(Style::TEXT),
                ColumnSpacing::Spaced,
            )
        };
        let cases = |delimiters, cell_style| {
            grid(
                delimiters,
                EnvironmentAlignment::Cases,
                EnvironmentArgument::None,
                Some(cell_style),
                ColumnSpacing::Spaced,
            )
        };
        Some(match name {
            "matrix" => matrix(None, EnvironmentArgument::None),
            "matrix*" => matrix(None, EnvironmentArgument::OptionalAlignment),
            "pmatrix" => matrix(Some((Some('('), Some(')'))), EnvironmentArgument::None),
            "pmatrix*" => matrix(
                Some((Some('('), Some(')'))),
                EnvironmentArgument::OptionalAlignment,
            ),
            "bmatrix" => matrix(Some((Some('['), Some(']'))), EnvironmentArgument::None),
            "bmatrix*" => matrix(
                Some((Some('['), Some(']'))),
                EnvironmentArgument::OptionalAlignment,
            ),
            "Bmatrix" => matrix(Some((Some('{'), Some('}'))), EnvironmentArgument::None),
            "Bmatrix*" => matrix(
                Some((Some('{'), Some('}'))),
                EnvironmentArgument::OptionalAlignment,
            ),
            "vmatrix" => matrix(Some((Some('|'), Some('|'))), EnvironmentArgument::None),
            "vmatrix*" => matrix(
                Some((Some('|'), Some('|'))),
                EnvironmentArgument::OptionalAlignment,
            ),
            "Vmatrix" => matrix(Some((Some('‖'), Some('‖'))), EnvironmentArgument::None),
            "Vmatrix*" => matrix(
                Some((Some('‖'), Some('‖'))),
                EnvironmentArgument::OptionalAlignment,
            ),
            "smallmatrix" => grid(
                None,
                EnvironmentAlignment::Columns,
                EnvironmentArgument::None,
                Some(Style::SCRIPT),
                ColumnSpacing::Spaced,
            ),
            // `array` inherits the surrounding style, unlike the matrix family.
            "array" => grid(
                None,
                EnvironmentAlignment::Specified,
                EnvironmentArgument::Columns,
                None,
                ColumnSpacing::Spaced,
            ),
            "subarray" => grid(
                None,
                EnvironmentAlignment::Specified,
                EnvironmentArgument::Columns,
                Some(Style::SCRIPT),
                ColumnSpacing::Spaced,
            ),
            "cases" => cases(Some((Some('{'), None)), Style::TEXT),
            "dcases" => cases(Some((Some('{'), None)), Style::DISPLAY),
            "rcases" => cases(Some((None, Some('}'))), Style::TEXT),
            "drcases" => cases(Some((Some('{'), Some('}'))), Style::DISPLAY),
            "aligned" | "align" | "align*" | "split" => grid(
                None,
                EnvironmentAlignment::Aligned,
                EnvironmentArgument::None,
                Some(Style::DISPLAY),
                ColumnSpacing::Spaced,
            ),
            // `alignat` sets its columns flush; the source supplies any space.
            "alignat" | "alignat*" | "alignedat" => grid(
                None,
                EnvironmentAlignment::Aligned,
                EnvironmentArgument::Pairs,
                Some(Style::DISPLAY),
                ColumnSpacing::Flush,
            ),
            "gather" | "gather*" | "gathered" => grid(
                None,
                EnvironmentAlignment::Center,
                EnvironmentArgument::None,
                Some(Style::DISPLAY),
                ColumnSpacing::Spaced,
            ),
            "equation" | "equation*" => Environment {
                delimiters: None,
                alignment: EnvironmentAlignment::Center,
                argument: EnvironmentArgument::None,
                formula: true,
                cell_style: Some(Style::TEXT),
                column_spacing: ColumnSpacing::Spaced,
            },
            _ => return None,
        })
    }
}

/// Text mode's escapes: `\{` and `\textbackslash` become the characters they
/// name, grouping braces disappear, and an unknown command stays literal.
fn decode_text(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(symbol) = chars.next() {
        match symbol {
            '\\' => {
                let Some(&next) = chars.peek() else {
                    out.push('\\');
                    break;
                };
                if next.is_ascii_alphabetic() {
                    let mut name = String::new();
                    while let Some(&letter) = chars.peek() {
                        if !letter.is_ascii_alphabetic() {
                            break;
                        }
                        name.push(letter);
                        chars.next();
                    }
                    match symbols::text_command(&name) {
                        Some(symbol) => out.push(symbol),
                        None => {
                            out.push('\\');
                            out.push_str(&name);
                        }
                    }
                } else {
                    chars.next();
                    match next {
                        '{' | '}' | '%' | '$' | '&' | '#' | '_' => out.push(next),
                        ' ' | ',' | ';' | ':' => out.push(' '),
                        '!' => {}
                        _ => {
                            out.push('\\');
                            out.push(next);
                        }
                    }
                }
            }
            '{' | '}' => {}
            '~' => out.push(' '),
            _ => out.push(symbol),
        }
    }
    out
}

/// A delimiter argument: empty or `.` is the null delimiter, a bare character
/// is itself, and a command goes through the delimiter table.
fn parse_delimiter_text(text: &str) -> Option<Option<char>> {
    let text = text.trim();
    if text.is_empty() || text == "." {
        return Some(None);
    }
    if let Some(name) = text.strip_prefix('\\') {
        return symbols::delimiter(name).map(Some);
    }
    let mut characters = text.chars();
    let symbol = characters.next()?;
    if characters.next().is_some() {
        return None;
    }
    match symbol {
        '(' | ')' | '[' | ']' | '|' | '/' | '<' | '>' | '⟨' | '⟩' | '⌊' | '⌋' | '⌈' | '⌉' => {
            Some(Some(symbol))
        }
        _ => None,
    }
}

/// `\genfrac`'s thickness also accepts a zero in a physical unit, as in the
/// common `0pt`, because zero is the same length in every unit.
fn parse_length_or_zero(text: &str) -> Option<Length> {
    if let Some(length) = parse_length(text) {
        return Some(length);
    }
    let number = text.trim_end_matches(|character: char| character.is_ascii_alphabetic());
    let value: f32 = number.parse().ok()?;
    (value == 0.0).then(|| Length::new(0, LengthUnit::Em))
}

/// Parses `\hspace`'s argument: a number and one of the units the model can
/// resolve without a device resolution.
fn parse_length(text: &str) -> Option<Length> {
    let text = text.trim();
    let split = text.find(|character: char| character.is_ascii_alphabetic())?;
    let (number, unit) = text.split_at(split);
    let unit = match unit {
        "em" => LengthUnit::Em,
        "ex" => LengthUnit::Ex,
        "mu" => LengthUnit::Mu,
        _ => return None,
    };
    let value: f32 = number.parse().ok()?;
    if !value.is_finite() {
        return None;
    }
    Some(Length::new((value * 1000.0).round() as i32, unit))
}

fn is_infix(name: &str) -> bool {
    matches!(name, "atop" | "over" | "choose" | "brack" | "brace")
}

/// The level a style switch selects; the cramped flag is cleared, since TeX's
/// `\displaystyle` and its siblings name the uncramped styles.
fn style_size(name: &str) -> Option<Size> {
    Some(match name {
        "displaystyle" => Size::Display,
        "textstyle" => Size::Text,
        "scriptstyle" => Size::Script,
        "scriptscriptstyle" => Size::ScriptScript,
        _ => return None,
    })
}

fn class_command(name: &str) -> Option<AtomClass> {
    Some(match name {
        "mathord" => AtomClass::Ord,
        "mathop" => AtomClass::Op,
        "mathbin" => AtomClass::Bin,
        "mathrel" => AtomClass::Rel,
        "mathopen" => AtomClass::Open,
        "mathclose" => AtomClass::Close,
        "mathpunct" => AtomClass::Punct,
        "mathinner" => AtomClass::Inner,
        _ => return None,
    })
}
