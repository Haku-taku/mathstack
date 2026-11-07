//! LaTeX math source to the `math-core` model and back.
//!
//! The parser accepts the constructs the layout engine knows and recovers in
//! place: an unknown command becomes an `Error` node so the rest of the
//! formula still parses and stays editable.

mod colors;
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
    LossKind, WriteLoss, WriteReport, to_latex, to_latex_with_report, to_latex_with_spans,
    to_latex_with_spans_and_report, to_latex_with_spans_and_spellings, to_latex_with_spellings,
    to_latex_with_spellings_and_report,
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

    /// The same output as [`Document::to_latex`], plus the places the writer
    /// had to approximate a model state with no exact spelling.
    pub fn to_latex_with_report(&self) -> (String, WriteReport) {
        writer::to_latex_with_spellings_and_report(&self.model, self.model.root(), &self.spellings)
    }

    /// Whether anything failed: a diagnostic or an `Error` node. A host uses
    /// this to mark the formula instead of rendering it silently.
    pub fn has_errors(&self) -> bool {
        !self.diagnostics.is_empty() || contains_error(&self.model, self.model.root())
    }
}

fn contains_error(model: &Model, id: NodeId) -> bool {
    matches!(model.kind(id), NodeKind::Error(_))
        || model
            .kind(id)
            .children()
            .iter()
            .any(|child| contains_error(model, child.id))
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
