//! Tokens with byte spans. Whitespace and `%` comments disappear; math-mode
//! spaces carry no meaning of their own.

use math_core::{Diagnostic, Origin, Span};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TokenKind {
    /// One source character: a letter, digit, operator or bracket.
    Symbol(char),
    /// A control sequence without its backslash; one character for `\{`,
    /// several for `\alpha`.
    Command(String),
    LBrace,
    RBrace,
    Sup,
    Sub,
    /// A construct the macro expander gave up on; the parser turns it into an
    /// error node.
    Error(String),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Token {
    pub kind: TokenKind,
    pub span: Span,
    /// Where the token came from; macro expansion marks body tokens.
    pub origin: Origin,
}

pub(crate) fn lex(source: &str) -> (Vec<Token>, Vec<Diagnostic>) {
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    let mut position = 0;

    while let Some(symbol) = source[position..].chars().next() {
        let start = position;
        position += symbol.len_utf8();
        match symbol {
            ' ' | '\t' | '\n' | '\r' => {}
            '%' => {
                while position < source.len() {
                    let next = source[position..].chars().next().unwrap_or('\n');
                    position += next.len_utf8();
                    if next == '\n' {
                        break;
                    }
                }
            }
            '{' => tokens.push(token(TokenKind::LBrace, start, position)),
            '}' => tokens.push(token(TokenKind::RBrace, start, position)),
            '^' => tokens.push(token(TokenKind::Sup, start, position)),
            '_' => tokens.push(token(TokenKind::Sub, start, position)),
            '\\' => {
                let name_start = position;
                let mut name = String::new();
                match source[position..].chars().next() {
                    Some(letter) if letter.is_ascii_alphabetic() => {
                        while let Some(letter) = source[position..].chars().next() {
                            if !letter.is_ascii_alphabetic() {
                                break;
                            }
                            name.push(letter);
                            position += letter.len_utf8();
                        }
                    }
                    Some(other) => {
                        name.push(other);
                        position += other.len_utf8();
                    }
                    None => {
                        diagnostics.push(Diagnostic::incomplete(
                            Span::new(start, position),
                            "backslash at end of input",
                        ));
                    }
                }
                debug_assert!(position >= name_start);
                tokens.push(token(TokenKind::Command(name), start, position));
            }
            other => tokens.push(token(TokenKind::Symbol(other), start, position)),
        }
    }
    (tokens, diagnostics)
}

fn token(kind: TokenKind, start: usize, end: usize) -> Token {
    Token {
        kind,
        span: Span::new(start, end),
        origin: Origin::Source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_and_comments_disappear() {
        let (tokens, diagnostics) = lex("a % comment\n+b");
        let kinds: Vec<_> = tokens.into_iter().map(|token| token.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Symbol('a'),
                TokenKind::Symbol('+'),
                TokenKind::Symbol('b')
            ]
        );
        assert!(diagnostics.is_empty());
    }
}
