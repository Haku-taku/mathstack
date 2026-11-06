//! Recursive descent over the token stream. Every construct keeps its source
//! span, and every failure recovers in place so a later edit still has a tree
//! to work with.

use crate::lexer::{Token, TokenKind};
use crate::spellings::{Spelling, Spellings};
use crate::symbols;
use math_core::{
    Alignment, Atom, Diagnostic, FractionRule, Length, LengthUnit, Limits, Model, ModelBuilder,
    NodeId, NodeKind, Origin, PhantomKind, Size, SpaceKind, Span, Variant,
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
}

enum RowStep {
    Close,
    StrayBrace(Span),
    StrayRight(Span),
    Infix(String, Span),
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
                TokenKind::Command(name) if name == "end" && stop == Stop::Cell => RowStep::Close,
                TokenKind::Command(name) if is_infix(name) => {
                    RowStep::Infix(name.clone(), token.span)
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
                    return self.parse_infix(&name, command_span, start, children, depth, stop);
                }
                RowStep::Item => {
                    let item = self.parse_item(depth);
                    children.push(item);
                }
            }
        }

        let end = children
            .last()
            .map_or_else(|| self.caret(), |child| self.span_of(*child).end);
        self.make(NodeKind::Row(children), Span::new(start, end.max(start)))
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
            "operatorname" => {
                self.advance();
                let limits = if matches!(
                    self.peek().map(|token| &token.kind),
                    Some(TokenKind::Symbol('*'))
                ) {
                    self.advance();
                    true
                } else {
                    false
                };
                match self.text_argument("\\operatorname", span) {
                    TextArgument::Text(name, span) => self.make(
                        NodeKind::Function {
                            name,
                            limits: limits.into(),
                        },
                        span,
                    ),
                    TextArgument::Unavailable(span) => {
                        self.make(NodeKind::Error("\\operatorname".to_owned()), span)
                    }
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
                            limits: limits.into(),
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

    /// `\begin{env} … \end{env}` for the matrix family and `cases`.
    fn parse_environment(&mut self, span: Span, depth: usize) -> NodeId {
        let (name, name_span) = match self.text_argument("\\begin", span) {
            TextArgument::Text(name, span) => (name.trim().to_owned(), span),
            TextArgument::Unavailable(span) => {
                return self.make(NodeKind::Error("\\begin".to_owned()), span);
            }
        };
        let Some(family) = Environment::lookup(&name) else {
            self.error(name_span, format!("unsupported environment `{name}`"));
            return self.make(NodeKind::Error(format!("\\begin{{{name}}}")), name_span);
        };

        let mut cells: Vec<Vec<NodeId>> = Vec::new();
        let mut columns = 0;
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

        columns = columns.max(1);
        let alignment = match family {
            Environment::Cases => vec![Alignment::Left; columns],
            // `aligned` alternates a right-aligned column and a left one.
            Environment::Aligned => (0..columns)
                .map(|column| {
                    if column % 2 == 0 {
                        Alignment::Right
                    } else {
                        Alignment::Left
                    }
                })
                .collect(),
            _ => Vec::new(),
        };
        let table_span = Span::new(span.start, end_span.end.max(span.end));
        let table = self.build_table(cells, columns, alignment, table_span, end_span.start);
        match family.delimiters() {
            None => table,
            Some((left, right)) => {
                let body = self.make(NodeKind::Row(vec![table]), table_span);
                self.make(NodeKind::Delimited { left, body, right }, table_span)
            }
        }
    }

    /// Pads rows to `columns` and wraps them in a table. `tail` is where the
    /// padding cells point when nothing was typed.
    fn build_table(
        &mut self,
        cells: Vec<Vec<NodeId>>,
        columns: usize,
        alignment: Vec<Alignment>,
        table_span: Span,
        tail: usize,
    ) -> NodeId {
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
        self.build_table(cells, columns, Vec::new(), table_span, end)
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

/// The table environments the grammar accepts.
#[derive(Clone, Copy)]
enum Environment {
    Matrix,
    Parenthesized,
    Bracketed,
    Braced,
    Determinant,
    Norm,
    Cases,
    Aligned,
}

impl Environment {
    fn lookup(name: &str) -> Option<Self> {
        Some(match name {
            "matrix" => Environment::Matrix,
            "pmatrix" => Environment::Parenthesized,
            "bmatrix" => Environment::Bracketed,
            "Bmatrix" => Environment::Braced,
            "vmatrix" => Environment::Determinant,
            "Vmatrix" => Environment::Norm,
            "cases" => Environment::Cases,
            "aligned" | "align" | "align*" => Environment::Aligned,
            _ => return None,
        })
    }

    /// The fences the parser wraps around the table; `None` is a bare grid.
    fn delimiters(self) -> Option<(Option<char>, Option<char>)> {
        match self {
            Environment::Matrix => None,
            Environment::Parenthesized => Some((Some('('), Some(')'))),
            Environment::Bracketed => Some((Some('['), Some(']'))),
            Environment::Braced => Some((Some('{'), Some('}'))),
            Environment::Determinant => Some((Some('|'), Some('|'))),
            Environment::Norm => Some((Some('‖'), Some('‖'))),
            Environment::Cases => Some((Some('{'), None)),
            Environment::Aligned => None,
        }
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
