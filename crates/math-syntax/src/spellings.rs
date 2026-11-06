//! The source spelling of nodes the writer would otherwise normalize.
//!
//! The model stores semantics, so `\pm` and `±` are the same atom. This side
//! table keeps the token a document actually used, keyed by node id, and the
//! writer emits it only while the node still has the shape it was read from.

use math_core::{AtomClass, DelimiterSize, Limits, NodeId, NodeKind, SpaceKind, Variant};
use std::collections::BTreeMap;

/// One node's original token and the shape it belonged to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spelling {
    pub(crate) text: String,
    pub(crate) kind: SpellingKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpellingKind {
    Atom {
        symbol: char,
        variant: Variant,
    },
    LargeOp {
        symbol: char,
        limits: Limits,
    },
    Function {
        name: String,
        limits: Limits,
    },
    Space(SpaceKind),
    Accent {
        symbol: char,
    },
    SizedDelimiter {
        symbol: char,
        size: DelimiterSize,
        class: AtomClass,
    },
    Middle {
        symbol: char,
    },
    Text {
        content: String,
    },
}

impl Spelling {
    /// The spellings a token can replace wholesale. An accent or a sized
    /// delimiter keeps its argument, so only its command is stored.
    pub(crate) fn from_kind(kind: &NodeKind, text: String) -> Option<Self> {
        let kind = match kind {
            NodeKind::Atom(atom) => SpellingKind::Atom {
                symbol: atom.symbol,
                variant: atom.variant,
            },
            NodeKind::LargeOp { symbol, limits } => SpellingKind::LargeOp {
                symbol: *symbol,
                limits: *limits,
            },
            NodeKind::Function { name, limits } => SpellingKind::Function {
                name: name.clone(),
                limits: *limits,
            },
            NodeKind::Space(kind) => SpellingKind::Space(*kind),
            NodeKind::Accent { symbol, .. } => SpellingKind::Accent { symbol: *symbol },
            NodeKind::SizedDelimiter {
                symbol,
                size,
                class,
            } => SpellingKind::SizedDelimiter {
                symbol: *symbol,
                size: *size,
                class: *class,
            },
            NodeKind::Middle { symbol } => SpellingKind::Middle { symbol: *symbol },
            NodeKind::Text(content) => SpellingKind::Text {
                content: content.clone(),
            },
            _ => return None,
        };
        Some(Spelling { text, kind })
    }

    /// Whether the node still means what the token said; a freed id may be
    /// reused, and an edited node may have a different symbol or variant.
    pub(crate) fn matches(&self, kind: &NodeKind) -> bool {
        match (&self.kind, kind) {
            (SpellingKind::Atom { symbol, variant }, NodeKind::Atom(atom)) => {
                *symbol == atom.symbol && *variant == atom.variant
            }
            (
                SpellingKind::LargeOp { symbol, limits },
                NodeKind::LargeOp {
                    symbol: current,
                    limits: current_limits,
                },
            ) => *symbol == *current && *limits == *current_limits,
            (
                SpellingKind::Function { name, limits },
                NodeKind::Function {
                    name: current,
                    limits: current_limits,
                },
            ) => name == current && limits == current_limits,
            (SpellingKind::Space(wanted), NodeKind::Space(current)) => wanted == current,
            (
                SpellingKind::Accent { symbol },
                NodeKind::Accent {
                    symbol: current, ..
                },
            ) => symbol == current,
            (
                SpellingKind::SizedDelimiter {
                    symbol,
                    size,
                    class,
                },
                NodeKind::SizedDelimiter {
                    symbol: current,
                    size: current_size,
                    class: current_class,
                },
            ) => symbol == current && size == current_size && class == current_class,
            (SpellingKind::Middle { symbol }, NodeKind::Middle { symbol: current }) => {
                symbol == current
            }
            (SpellingKind::Text { content }, NodeKind::Text(current)) => content == current,
            _ => false,
        }
    }
}

/// Source spellings by node, kept beside a `Model` rather than inside it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Spellings {
    entries: BTreeMap<NodeId, Spelling>,
    /// The environment a table, its fenced wrapper, or an unwrapped
    /// `equation` body was written as; the model only keeps its shape.
    environments: BTreeMap<NodeId, String>,
}

impl Spellings {
    pub fn get(&self, node: NodeId) -> Option<&Spelling> {
        self.entries.get(&node)
    }

    pub fn insert(&mut self, node: NodeId, spelling: Spelling) {
        self.entries.insert(node, spelling);
    }

    /// The environment name a node was parsed from, when the writer cannot
    /// recover it from the node's shape.
    pub fn environment(&self, node: NodeId) -> Option<&str> {
        self.environments.get(&node).map(String::as_str)
    }

    pub fn set_environment(&mut self, node: NodeId, name: String) {
        self.environments.insert(node, name);
    }

    /// Copies `from`'s spelling to `to`, for moving a parsed subtree.
    pub fn copy(&mut self, from: NodeId, to: NodeId) {
        if let Some(spelling) = self.entries.get(&from).cloned() {
            self.entries.insert(to, spelling);
        }
        if let Some(environment) = self.environments.get(&from).cloned() {
            self.environments.insert(to, environment);
        }
    }

    pub fn remove(&mut self, node: NodeId) {
        self.entries.remove(&node);
        self.environments.remove(&node);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
