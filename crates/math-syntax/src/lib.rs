//! LaTeX math source to the `math-core` model and back.
//!
//! The parser accepts the constructs the layout engine knows and recovers in
//! place: an unknown command becomes an `Error` node so the rest of the
//! formula still parses and stays editable.

mod commands;
mod lexer;
mod macros;
mod parser;
mod spellings;
mod symbols;
mod writer;

use math_core::Diagnostic;

pub use commands::{CommandShape, command_shape, command_template, delimiter_symbol};
pub use math_core::{Model, Node, NodeId, NodeKind, Span};
pub use spellings::{Spelling, Spellings};
pub use writer::{
    to_latex, to_latex_with_spans, to_latex_with_spans_and_spellings, to_latex_with_spellings,
};

/// A parsed formula with the source it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    pub source: String,
    pub model: Model,
    /// The tokens the model would otherwise normalize (`\pm`, `\le`, …).
    pub spellings: Spellings,
    /// Lex and parse problems, in source order.
    pub diagnostics: Vec<Diagnostic>,
}

impl Document {
    pub fn root(&self) -> NodeId {
        self.model.root()
    }

    /// Exports the model, emitting each node's original token when it still
    /// has the shape that token described.
    pub fn to_latex(&self) -> String {
        to_latex_with_spellings(&self.model, self.model.root(), &self.spellings)
    }

    /// Whether anything failed: a diagnostic or an `Error` node. A host uses
    /// this to mark the formula instead of rendering it silently.
    pub fn has_errors(&self) -> bool {
        !self.diagnostics.is_empty() || contains_error(&self.model, self.model.root())
    }
}

fn contains_error(model: &Model, id: NodeId) -> bool {
    match model.kind(id) {
        NodeKind::Error(_) => true,
        NodeKind::Row(children) => children.iter().any(|child| contains_error(model, *child)),
        NodeKind::Scripts { base, sup, sub } => {
            contains_error(model, *base)
                || sup.is_some_and(|sup| contains_error(model, sup))
                || sub.is_some_and(|sub| contains_error(model, sub))
        }
        NodeKind::Fraction {
            numerator,
            denominator,
            ..
        } => contains_error(model, *numerator) || contains_error(model, *denominator),
        NodeKind::Radical { body, degree } => {
            contains_error(model, *body)
                || degree.is_some_and(|degree| contains_error(model, degree))
        }
        NodeKind::Delimited { body, .. } | NodeKind::Accent { base: body, .. } => {
            contains_error(model, *body)
        }
        NodeKind::Text(_)
        | NodeKind::Composition(_)
        | NodeKind::Atom(_)
        | NodeKind::LargeOp { .. }
        | NodeKind::Function { .. }
        | NodeKind::Space(_)
        | NodeKind::Hspace(_)
        | NodeKind::Middle { .. }
        | NodeKind::SizedDelimiter { .. } => false,
        NodeKind::Overline { body } | NodeKind::Underline { body } => contains_error(model, *body),
        NodeKind::Phantom { body, .. }
        | NodeKind::Smash { body, .. }
        | NodeKind::Boxed { body }
        | NodeKind::Cancel { body, .. }
        | NodeKind::Styled { body, .. }
        | NodeKind::Class { body, .. } => contains_error(model, *body),
        NodeKind::Arrow { over, under, .. } => {
            over.is_some_and(|over| contains_error(model, over))
                || under.is_some_and(|under| contains_error(model, under))
        }
        NodeKind::HorizontalBrace { body, .. } => contains_error(model, *body),
        NodeKind::Table { rows, .. } => rows.iter().any(|row| contains_error(model, *row)),
        NodeKind::TableRow { cells } => cells.iter().any(|cell| contains_error(model, *cell)),
        NodeKind::OverUnder { base, over, under } => {
            contains_error(model, *base)
                || over.is_some_and(|over| contains_error(model, over))
                || under.is_some_and(|under| contains_error(model, under))
        }
    }
}

pub fn parse(source: &str) -> Document {
    let (tokens, mut diagnostics) = lexer::lex(source);
    let tokens = macros::expand(tokens, &mut diagnostics);
    let (model, spellings, diagnostics) = parser::parse(source, tokens, diagnostics);
    Document {
        source: source.to_owned(),
        model,
        spellings,
        diagnostics,
    }
}
