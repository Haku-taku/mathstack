//! Token-list macro expansion before parsing.
//!
//! The expander implements the small subset the design allows: `\def` and
//! `\newcommand`/`\renewcommand` with `#1`…`#9`, no delimited parameter text,
//! no `\edef`, no catcodes, and definitions scoped to the whole formula.
//! Arguments keep their source spans; all other expanded tokens take the call
//! site's span, so every byte remains covered by a call that produced it.

use crate::lexer::{Token, TokenKind};
use math_core::{Diagnostic, Origin, Span};
use std::collections::{HashMap, VecDeque};

const MAX_EXPANSIONS: usize = 1_000;
const MAX_EXPANDED_TOKENS: usize = 100_000;

struct Macro {
    parameters: usize,
    body: Vec<Token>,
    /// The definition command's span, recorded on every expanded node.
    definition: Span,
}

pub(crate) fn expand(tokens: Vec<Token>, diagnostics: &mut Vec<Diagnostic>) -> Vec<Token> {
    let mut input: VecDeque<Token> = tokens.into();
    let mut macros: HashMap<String, Macro> = HashMap::new();
    let mut output = Vec::new();
    let mut expansions = 0;
    let mut expanded_tokens = 0;
    let mut limit_reported = false;

    while let Some(token) = input.pop_front() {
        let TokenKind::Command(name) = &token.kind else {
            output.push(token);
            continue;
        };
        match name.as_str() {
            "def" => define_def(token.span, &mut input, &mut macros, diagnostics),
            "newcommand" | "renewcommand" => {
                define_new(name, token.span, &mut input, &mut macros, diagnostics);
            }
            _ => {
                let Some(definition) = macros.get(name) else {
                    output.push(token);
                    continue;
                };
                let parameters = definition.parameters;
                let body = definition.body.clone();
                let mut arguments = Vec::with_capacity(parameters);
                let mut complete = true;
                for _ in 0..parameters {
                    match collect_argument(&mut input, diagnostics, token.span) {
                        Some(argument) => arguments.push(argument),
                        None => {
                            complete = false;
                            arguments.push(Vec::new());
                        }
                    }
                }
                if !complete {
                    diagnostics.push(Diagnostic::new(
                        token.span,
                        format!("missing argument for \\{name}"),
                    ));
                }
                expansions += 1;
                let replacement = substitute(&body, &arguments, definition.definition, token.span);
                expanded_tokens += replacement.len();
                if expansions > MAX_EXPANSIONS || expanded_tokens > MAX_EXPANDED_TOKENS {
                    if !limit_reported {
                        limit_reported = true;
                        diagnostics.push(Diagnostic::new(
                            token.span,
                            "macro expansion limit exceeded",
                        ));
                    }
                    output.push(Token {
                        kind: TokenKind::Error(format!("\\{name}")),
                        span: token.span,
                        origin: token.origin,
                    });
                    continue;
                }
                for expanded in replacement.into_iter().rev() {
                    input.push_front(expanded);
                }
            }
        }
    }
    output
}

fn define_def(
    span: Span,
    input: &mut VecDeque<Token>,
    macros: &mut HashMap<String, Macro>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(name_token) = input.pop_front() else {
        diagnostics.push(Diagnostic::incomplete(
            span,
            "missing macro name after \\def",
        ));
        return;
    };
    let TokenKind::Command(name) = &name_token.kind else {
        diagnostics.push(Diagnostic::new(
            name_token.span,
            "expected a control sequence after \\def",
        ));
        return;
    };
    let name = name.clone();
    let mut parameters = 0;
    loop {
        match input.front().map(|token| &token.kind) {
            Some(TokenKind::LBrace) => break,
            Some(TokenKind::Symbol('#')) => {
                input.pop_front();
                match input.pop_front() {
                    Some(Token {
                        kind: TokenKind::Symbol(digit),
                        ..
                    }) if digit.is_ascii_digit() => {
                        let number = digit.to_digit(10).unwrap_or(0) as usize;
                        if number != parameters + 1 {
                            diagnostics.push(Diagnostic::new(
                                name_token.span,
                                "macro parameters must be numbered consecutively",
                            ));
                        }
                        parameters = parameters.max(number);
                    }
                    _ => diagnostics.push(Diagnostic::new(
                        name_token.span,
                        "expected a parameter number after #",
                    )),
                }
            }
            Some(_) => {
                diagnostics.push(Diagnostic::new(
                    name_token.span,
                    "unsupported text in a \\def parameter list",
                ));
                input.pop_front();
            }
            None => {
                diagnostics.push(Diagnostic::incomplete(
                    name_token.span,
                    "missing macro body",
                ));
                return;
            }
        }
    }
    let Some(body) = collect_group(input) else {
        diagnostics.push(Diagnostic::incomplete(
            name_token.span,
            "missing macro body",
        ));
        return;
    };
    check_parameters(&body, parameters, name_token.span, diagnostics);
    macros.insert(
        name,
        Macro {
            parameters,
            body,
            definition: span,
        },
    );
}

fn define_new(
    command: &str,
    span: Span,
    input: &mut VecDeque<Token>,
    macros: &mut HashMap<String, Macro>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(name) = take_macro_name(span, input, diagnostics) else {
        return;
    };
    let mut parameters = 0;
    if matches!(
        input.front().map(|token| &token.kind),
        Some(TokenKind::Symbol('['))
    ) {
        input.pop_front();
        let mut digits = String::new();
        while let Some(token) = input.pop_front() {
            match token.kind {
                TokenKind::Symbol(']') => break,
                TokenKind::Symbol(digit) if digit.is_ascii_digit() => digits.push(digit),
                _ => diagnostics.push(Diagnostic::new(
                    token.span,
                    "unsupported text in a macro parameter count",
                )),
            }
        }
        parameters = digits.parse().unwrap_or(0);
    }
    let Some(body) = collect_group(input) else {
        diagnostics.push(Diagnostic::incomplete(span, "missing macro body"));
        return;
    };
    let exists = macros.contains_key(&name);
    match command {
        "newcommand" if exists => {
            diagnostics.push(Diagnostic::new(
                span,
                format!("\\{name} is already defined"),
            ));
            return;
        }
        "renewcommand" if !exists => {
            diagnostics.push(Diagnostic::new(span, format!("\\{name} is not defined")))
        }
        _ => {}
    }
    check_parameters(&body, parameters, span, diagnostics);
    macros.insert(
        name,
        Macro {
            parameters,
            body,
            definition: span,
        },
    );
}

fn check_parameters(
    body: &[Token],
    parameters: usize,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for pair in body.windows(2) {
        let TokenKind::Symbol('#') = pair[0].kind else {
            continue;
        };
        let TokenKind::Symbol(digit) = pair[1].kind else {
            continue;
        };
        if let Some(number) = digit.to_digit(10)
            && number as usize > parameters
        {
            diagnostics.push(Diagnostic::new(
                span,
                format!("parameter #{number} exceeds the macro's {parameters} parameter(s)"),
            ));
        }
    }
}

fn take_macro_name(
    span: Span,
    input: &mut VecDeque<Token>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    let Some(token) = input.pop_front() else {
        diagnostics.push(Diagnostic::incomplete(span, "missing macro name"));
        return None;
    };
    match token.kind {
        TokenKind::Command(name) => Some(name),
        TokenKind::LBrace => {
            let Some(Token {
                kind: TokenKind::Command(name),
                ..
            }) = input.pop_front()
            else {
                diagnostics.push(Diagnostic::new(token.span, "expected a control sequence"));
                return None;
            };
            if !matches!(
                input.front().map(|token| &token.kind),
                Some(TokenKind::RBrace)
            ) {
                diagnostics.push(Diagnostic::new(
                    token.span,
                    "expected `}` after the macro name",
                ));
            } else {
                input.pop_front();
            }
            Some(name)
        }
        _ => {
            diagnostics.push(Diagnostic::new(token.span, "expected a control sequence"));
            None
        }
    }
}

fn collect_argument(
    input: &mut VecDeque<Token>,
    diagnostics: &mut Vec<Diagnostic>,
    call: Span,
) -> Option<Vec<Token>> {
    let first = input.pop_front()?;
    if !matches!(first.kind, TokenKind::LBrace) {
        return Some(vec![first]);
    }
    let mut depth = 0;
    let mut argument = Vec::new();
    while let Some(token) = input.pop_front() {
        match token.kind {
            TokenKind::RBrace if depth == 0 => return Some(argument),
            TokenKind::RBrace => {
                depth -= 1;
                argument.push(token);
            }
            TokenKind::LBrace => {
                depth += 1;
                argument.push(token);
            }
            _ => argument.push(token),
        }
    }
    diagnostics.push(Diagnostic::incomplete(call, "unclosed macro argument"));
    Some(argument)
}

fn collect_group(input: &mut VecDeque<Token>) -> Option<Vec<Token>> {
    if !matches!(
        input.front().map(|token| &token.kind),
        Some(TokenKind::LBrace)
    ) {
        return None;
    }
    input.pop_front();
    let mut depth = 0;
    let mut body = Vec::new();
    while let Some(token) = input.pop_front() {
        match token.kind {
            TokenKind::RBrace if depth == 0 => return Some(body),
            TokenKind::RBrace => {
                depth -= 1;
                body.push(token);
            }
            TokenKind::LBrace => {
                depth += 1;
                body.push(token);
            }
            _ => body.push(token),
        }
    }
    None
}

fn substitute(
    body: &[Token],
    arguments: &[Vec<Token>],
    definition: Span,
    call: Span,
) -> Vec<Token> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < body.len() {
        let parameter = match body.get(index) {
            Some(Token {
                kind: TokenKind::Symbol('#'),
                ..
            }) => match body.get(index + 1) {
                Some(Token {
                    kind: TokenKind::Symbol(digit),
                    ..
                }) => digit.to_digit(10).filter(|number| *number >= 1),
                _ => None,
            },
            _ => None,
        };
        if let Some(number) = parameter {
            if let Some(argument) = arguments.get(number as usize - 1) {
                out.extend(argument.iter().cloned());
            }
            index += 2;
            continue;
        }
        let mut token = body[index].clone();
        token.span = call;
        token.origin = Origin::Macro { definition, call };
        out.push(token);
        index += 1;
    }
    out
}
