//! The model: an id-stable arena of nodes, and what the parser produces.
//!
//! Children are [`NodeId`]s, so a layout or a caret path refers to a node by
//! identity rather than by walking a value tree. Every node carries the byte
//! span it came from and where that span came from (source or macro).

use crate::atom::{Atom, Variant};
use crate::class::AtomClass;
use crate::color::Color;
use crate::span::Span;
use crate::style::{Size, Style};

/// A spacing command's width, in em.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpaceKind {
    Thin,
    Medium,
    Thick,
    NegativeThin,
    Half,
    Quad,
    DoubleQuad,
    Normal,
}

impl SpaceKind {
    pub const fn em(self) -> f32 {
        match self {
            SpaceKind::Thin => 3.0 / 18.0,
            SpaceKind::NegativeThin => -3.0 / 18.0,
            SpaceKind::Medium => 4.0 / 18.0,
            SpaceKind::Thick => 5.0 / 18.0,
            SpaceKind::Half => 0.5,
            SpaceKind::Quad => 1.0,
            SpaceKind::DoubleQuad => 2.0,
            SpaceKind::Normal => 1.0 / 3.0,
        }
    }
}

/// A unit `\hspace` accepts. Physical units are not represented because the
/// model has no resolution to convert them with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LengthUnit {
    Em,
    Ex,
    Mu,
}

/// A length in thousandths of `unit`; integer storage keeps nodes comparable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Length {
    pub milli: i32,
    pub unit: LengthUnit,
}

impl Length {
    pub const fn new(milli: i32, unit: LengthUnit) -> Self {
        Length { milli, unit }
    }

    /// The length in em, given the em size and the font's x-height in em.
    pub fn em(self, em: f32, x_height: f32) -> f32 {
        let scale = match self.unit {
            LengthUnit::Em => 1.0,
            LengthUnit::Ex => x_height,
            LengthUnit::Mu => 1.0 / 18.0,
        };
        self.milli as f32 / 1000.0 * scale * em
    }
}

/// What separates a fraction's parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FractionRule {
    /// No rule (`\atop`).
    None,
    /// The font's `fractionRuleThickness`.
    Default,
    /// A rule of the given length (`\genfrac`).
    Custom(Length),
}

/// Which dimensions `\phantom` keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhantomKind {
    Full,
    Horizontal,
    Vertical,
}

/// The four fixed delimiter sizes of the `\big` family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelimiterSize {
    Big,
    Bigger,
    BiggerStill,
    Biggest,
}

impl DelimiterSize {
    /// The requested height in em (KaTeX's `sizeToMaxHeight`).
    pub const fn height(self) -> f32 {
        match self {
            DelimiterSize::Big => 1.2,
            DelimiterSize::Bigger => 1.8,
            DelimiterSize::BiggerStill => 2.4,
            DelimiterSize::Biggest => 3.0,
        }
    }
}

/// Where an operator's scripts go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limits {
    /// No suffix: the symbol table's answer for display style. The payload is
    /// that answer, so layout needs no symbol table of its own.
    Default { display_limits: bool },
    /// `\limits`: above and below in every style.
    AboveBelow,
    /// `\nolimits`: at the operator's side in every style.
    Beside,
}

impl Limits {
    /// Whether this placement puts the scripts above and below in `display`.
    pub fn above_below(self, display: bool) -> bool {
        match self {
            Limits::Default { display_limits } => display_limits && display,
            Limits::AboveBelow => true,
            Limits::Beside => false,
        }
    }
}

impl From<bool> for Limits {
    /// `true` when the operator's table entry puts scripts above and below in
    /// display style.
    fn from(display_limits: bool) -> Self {
        Limits::Default { display_limits }
    }
}

/// A node's address in a [`Model`]. Ids are stable until the node is deleted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(u32);

impl NodeId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Where a node's span points.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Origin {
    /// Bytes of the document itself.
    #[default]
    Source,
    /// Bytes of a macro body; the definition may live outside the document, so
    /// the expansion is read-only and the call site is what an edit targets.
    Macro { definition: Span, call: Span },
    /// A node the engine created, such as an empty slot.
    Synthetic,
}

/// The named position of a child inside its parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Slot {
    /// A row's children, or the single body slot of a unary node.
    Body,
    Base,
    Numerator,
    Denominator,
    Superscript,
    Subscript,
    Degree,
    Over,
    Under,
    /// A table's rows.
    Row,
    /// A table row's cells.
    Cell,
    /// One of `\mathchoice`'s branches, named by the style that selects it.
    Choice(Size),
}

/// How a table column places its cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Alignment {
    Left,
    #[default]
    Center,
    Right,
}

/// How much space separates a table's columns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColumnSpacing {
    /// Half an em at every boundary, the matrix default.
    #[default]
    Spaced,
    /// No space beyond the cells: `alignat` leaves separation to the source.
    Flush,
}

/// One child of a node, with the slot it occupies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChildSlot {
    pub slot: Slot,
    pub id: NodeId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub kind: NodeKind,
    pub span: Span,
    pub origin: Origin,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Atom(Atom),
    Row(Vec<NodeId>),
    Scripts {
        base: NodeId,
        sup: Option<NodeId>,
        sub: Option<NodeId>,
    },
    Fraction {
        numerator: NodeId,
        denominator: NodeId,
        rule: FractionRule,
        /// A forced style level (`\dfrac`, `\genfrac`); `None` inherits.
        style: Option<Size>,
    },
    Radical {
        body: NodeId,
        degree: Option<NodeId>,
    },
    /// `\left…\right`; `None` is TeX's null delimiter.
    Delimited {
        left: Option<char>,
        body: NodeId,
        right: Option<char>,
    },
    /// A large operator, with its default placement (`\sum` limits, `\int`
    /// side) and any `\limits`/`\nolimits` suffix.
    LargeOp {
        symbol: char,
        limits: Limits,
    },
    /// A combining mark drawn above its base (`\hat`, `\bar`, …).
    Accent {
        base: NodeId,
        symbol: char,
    },
    /// Text mode (`\text{…}`): upright, spaces kept.
    Text(String),
    /// A live IME preedit: laid out like text, never written, never undoable.
    Composition(String),
    /// A named operator such as `\sin`, or `\operatorname{…}`, with the same
    /// placement rules as a large operator.
    Function {
        name: String,
        limits: Limits,
    },
    /// A spacing command; its width does not participate in atom glue.
    Space(SpaceKind),
    /// `\hspace`: an explicit length, also outside atom glue.
    Hspace(Length),
    /// A rule across the top of its base (`\overline`).
    Overline {
        body: NodeId,
    },
    /// A rule across the bottom of its base (`\underline`).
    Underline {
        body: NodeId,
    },
    /// A delimiter inside `\left…\right` that grows with the whole group.
    Middle {
        symbol: char,
    },
    /// A `\big`-family fence, grown to a fixed height.
    SizedDelimiter {
        symbol: char,
        size: DelimiterSize,
        class: AtomClass,
    },
    /// `\phantom` and its one-axis variants: space without ink.
    Phantom {
        body: NodeId,
        kind: PhantomKind,
    },
    /// `\smash`: ink without space on the chosen axes.
    Smash {
        body: NodeId,
        top: bool,
        bottom: bool,
    },
    /// `\colorbox`/`\fcolorbox`: text on a filled rectangle, with an
    /// optional border whose color is separate from the background.
    ColorBox {
        background: Color,
        border: Option<Color>,
        body: NodeId,
    },
    /// `\boxed`: the base inside a rule frame.
    Boxed {
        body: NodeId,
    },
    /// `\overset` / `\underset`: a script above or below an arbitrary base.
    OverUnder {
        base: NodeId,
        over: Option<NodeId>,
        under: Option<NodeId>,
    },
    /// A horizontally stretched arrow with optional labels (`\xrightarrow`).
    Arrow {
        symbol: char,
        over: Option<NodeId>,
        under: Option<NodeId>,
    },
    /// A horizontal brace stretched across its body (`\overbrace`).
    HorizontalBrace {
        body: NodeId,
        over: bool,
    },
    /// `\cancel` and its two-direction variants: a diagonal stroked over the
    /// body. `up` rises left to right, `down` falls.
    Cancel {
        body: NodeId,
        up: bool,
        down: bool,
    },
    /// A style switch applied to the rest of its group (`\displaystyle`).
    Styled {
        size: Size,
        body: NodeId,
    },
    /// A font command over an editable body (`\mathbb`, `\mathbf`, …). Atoms
    /// that carry no variant of their own take this one; an inner command
    /// overrides an outer one.
    Variant {
        variant: Variant,
        body: NodeId,
    },
    /// A color over a body: `\textcolor`, or the scope of a `\color`
    /// declaration.
    Color {
        color: Color,
        body: NodeId,
    },
    /// An explicit atom class over a body (`\mathop{…}` and its siblings).
    Class {
        class: AtomClass,
        body: NodeId,
        limits: Limits,
    },
    /// `\mathchoice`: one branch per style level. Layout lays out the branch
    /// the current style selects; the others stay in the model for editing and
    /// writing.
    MathChoice {
        display: NodeId,
        text: NodeId,
        script: NodeId,
        script_script: NodeId,
    },
    /// A stretched arrow below its body (`\underrightarrow`): the body keeps
    /// the baseline and the arrow hangs under it.
    UnderArrow {
        symbol: char,
        body: NodeId,
    },
    /// A matrix-style table: rows of cells sharing column widths.
    Table {
        rows: Vec<NodeId>,
        columns: usize,
        /// One entry per column; shorter lists centre the remaining columns.
        alignment: Vec<Alignment>,
        /// The style the cells are set in; `None` inherits the table's.
        cell_style: Option<Style>,
        /// Vertical rules at the column boundaries, left to right: one count
        /// per boundary, so `columns + 1` entries. Empty when there are none.
        column_rules: Vec<u8>,
        /// How adjacent columns are separated.
        column_spacing: ColumnSpacing,
    },
    /// One table row; its children are cells (each a row of its own).
    TableRow {
        cells: Vec<NodeId>,
    },
    /// Source the parser could not use; layout draws its raw text upright.
    Error(String),
}

impl NodeKind {
    /// Whether the node is glue: it takes width but no part in atom classes.
    pub fn is_space(&self) -> bool {
        matches!(self, NodeKind::Space(_) | NodeKind::Hspace(_))
    }

    /// Whether `slot` holds a variable-length list rather than one child.
    pub fn slot_is_list(&self, slot: Slot) -> bool {
        matches!(
            (self, slot),
            (NodeKind::Row(_), Slot::Body)
                | (NodeKind::Table { .. }, Slot::Row)
                | (NodeKind::TableRow { .. }, Slot::Cell)
        )
    }

    /// Whether `slot` is a fixed-arity grid position. Deleting a cell or row
    /// would change the table's shape, so editing clears its contents instead.
    pub fn slot_is_structure(&self, slot: Slot) -> bool {
        matches!(
            (self, slot),
            (NodeKind::Table { .. }, Slot::Row) | (NodeKind::TableRow { .. }, Slot::Cell)
        )
    }

    /// Whether `slot` may be empty, so deleting its child removes the slot
    /// itself; required slots keep an empty row for the caret.
    pub fn slot_is_optional(&self, slot: Slot) -> bool {
        matches!(
            (self, slot),
            (
                NodeKind::Scripts { .. },
                Slot::Superscript | Slot::Subscript
            ) | (NodeKind::Radical { .. }, Slot::Degree)
                | (
                    NodeKind::OverUnder { .. } | NodeKind::Arrow { .. },
                    Slot::Over | Slot::Under
                )
        )
    }

    /// The node's children in reading order, each with its slot.
    pub fn children(&self) -> Vec<ChildSlot> {
        match self {
            NodeKind::Row(children) => children
                .iter()
                .map(|id| ChildSlot {
                    slot: Slot::Body,
                    id: *id,
                })
                .collect(),
            NodeKind::Scripts { base, sup, sub } => {
                let mut children = vec![ChildSlot {
                    slot: Slot::Base,
                    id: *base,
                }];
                if let Some(sup) = sup {
                    children.push(ChildSlot {
                        slot: Slot::Superscript,
                        id: *sup,
                    });
                }
                if let Some(sub) = sub {
                    children.push(ChildSlot {
                        slot: Slot::Subscript,
                        id: *sub,
                    });
                }
                children
            }
            NodeKind::MathChoice {
                display,
                text,
                script,
                script_script,
            } => vec![
                ChildSlot {
                    slot: Slot::Choice(Size::Display),
                    id: *display,
                },
                ChildSlot {
                    slot: Slot::Choice(Size::Text),
                    id: *text,
                },
                ChildSlot {
                    slot: Slot::Choice(Size::Script),
                    id: *script,
                },
                ChildSlot {
                    slot: Slot::Choice(Size::ScriptScript),
                    id: *script_script,
                },
            ],
            NodeKind::Fraction {
                numerator,
                denominator,
                ..
            } => vec![
                ChildSlot {
                    slot: Slot::Numerator,
                    id: *numerator,
                },
                ChildSlot {
                    slot: Slot::Denominator,
                    id: *denominator,
                },
            ],
            NodeKind::Radical { body, degree } => {
                let mut children = vec![ChildSlot {
                    slot: Slot::Body,
                    id: *body,
                }];
                if let Some(degree) = degree {
                    children.push(ChildSlot {
                        slot: Slot::Degree,
                        id: *degree,
                    });
                }
                children
            }
            NodeKind::Delimited { body, .. }
            | NodeKind::Accent { base: body, .. }
            | NodeKind::Overline { body }
            | NodeKind::Underline { body }
            | NodeKind::Phantom { body, .. }
            | NodeKind::Smash { body, .. }
            | NodeKind::ColorBox { body, .. }
            | NodeKind::Boxed { body }
            | NodeKind::Cancel { body, .. }
            | NodeKind::Styled { body, .. }
            | NodeKind::Variant { body, .. }
            | NodeKind::Color { body, .. }
            | NodeKind::Class { body, .. }
            | NodeKind::UnderArrow { body, .. } => vec![ChildSlot {
                slot: if matches!(self, NodeKind::Accent { .. }) {
                    Slot::Base
                } else {
                    Slot::Body
                },
                id: *body,
            }],
            NodeKind::OverUnder { base, over, under } => {
                let mut children = vec![ChildSlot {
                    slot: Slot::Base,
                    id: *base,
                }];
                if let Some(over) = over {
                    children.push(ChildSlot {
                        slot: Slot::Over,
                        id: *over,
                    });
                }
                if let Some(under) = under {
                    children.push(ChildSlot {
                        slot: Slot::Under,
                        id: *under,
                    });
                }
                children
            }
            NodeKind::Arrow { over, under, .. } => {
                let mut children = Vec::new();
                if let Some(over) = over {
                    children.push(ChildSlot {
                        slot: Slot::Over,
                        id: *over,
                    });
                }
                if let Some(under) = under {
                    children.push(ChildSlot {
                        slot: Slot::Under,
                        id: *under,
                    });
                }
                children
            }
            NodeKind::HorizontalBrace { body, .. } => vec![ChildSlot {
                slot: Slot::Body,
                id: *body,
            }],
            NodeKind::Table { rows, .. } => rows
                .iter()
                .map(|id| ChildSlot {
                    slot: Slot::Row,
                    id: *id,
                })
                .collect(),
            NodeKind::TableRow { cells } => cells
                .iter()
                .map(|id| ChildSlot {
                    slot: Slot::Cell,
                    id: *id,
                })
                .collect(),
            NodeKind::Atom(_)
            | NodeKind::LargeOp { .. }
            | NodeKind::Text(_)
            | NodeKind::Composition(_)
            | NodeKind::Function { .. }
            | NodeKind::Space(_)
            | NodeKind::Hspace(_)
            | NodeKind::Middle { .. }
            | NodeKind::SizedDelimiter { .. }
            | NodeKind::Error(_) => Vec::new(),
        }
    }

    /// Rebuilds the kind with every child referenced through `map`.
    ///
    /// A host uses this to move a parsed subtree into another model, where the
    /// ids mean nothing; the mapping function allocates the children first.
    pub fn map_children(&self, mut map: impl FnMut(NodeId) -> NodeId) -> NodeKind {
        match self {
            NodeKind::Atom(atom) => NodeKind::Atom(*atom),
            NodeKind::Row(children) => NodeKind::Row(children.iter().map(|id| map(*id)).collect()),
            NodeKind::Scripts { base, sup, sub } => NodeKind::Scripts {
                base: map(*base),
                sup: sup.map(&mut map),
                sub: sub.map(&mut map),
            },
            NodeKind::Fraction {
                numerator,
                denominator,
                rule,
                style,
            } => NodeKind::Fraction {
                numerator: map(*numerator),
                denominator: map(*denominator),
                rule: *rule,
                style: *style,
            },
            NodeKind::Radical { body, degree } => NodeKind::Radical {
                body: map(*body),
                degree: degree.map(&mut map),
            },
            NodeKind::Delimited { left, body, right } => NodeKind::Delimited {
                left: *left,
                body: map(*body),
                right: *right,
            },
            NodeKind::LargeOp { symbol, limits } => NodeKind::LargeOp {
                symbol: *symbol,
                limits: *limits,
            },
            NodeKind::Accent { base, symbol } => NodeKind::Accent {
                base: map(*base),
                symbol: *symbol,
            },
            NodeKind::Text(text) => NodeKind::Text(text.clone()),
            NodeKind::Composition(text) => NodeKind::Composition(text.clone()),
            NodeKind::Function { name, limits } => NodeKind::Function {
                name: name.clone(),
                limits: *limits,
            },
            NodeKind::Space(kind) => NodeKind::Space(*kind),
            NodeKind::Hspace(length) => NodeKind::Hspace(*length),
            NodeKind::Overline { body } => NodeKind::Overline { body: map(*body) },
            NodeKind::Underline { body } => NodeKind::Underline { body: map(*body) },
            NodeKind::Middle { symbol } => NodeKind::Middle { symbol: *symbol },
            NodeKind::SizedDelimiter {
                symbol,
                size,
                class,
            } => NodeKind::SizedDelimiter {
                symbol: *symbol,
                size: *size,
                class: *class,
            },
            NodeKind::Phantom { body, kind } => NodeKind::Phantom {
                body: map(*body),
                kind: *kind,
            },
            NodeKind::Smash { body, top, bottom } => NodeKind::Smash {
                body: map(*body),
                top: *top,
                bottom: *bottom,
            },
            NodeKind::ColorBox {
                background,
                border,
                body,
            } => NodeKind::ColorBox {
                background: *background,
                border: *border,
                body: map(*body),
            },
            NodeKind::Boxed { body } => NodeKind::Boxed { body: map(*body) },
            NodeKind::OverUnder { base, over, under } => NodeKind::OverUnder {
                base: map(*base),
                over: over.map(&mut map),
                under: under.map(&mut map),
            },
            NodeKind::Arrow {
                symbol,
                over,
                under,
            } => NodeKind::Arrow {
                symbol: *symbol,
                over: over.map(&mut map),
                under: under.map(&mut map),
            },
            NodeKind::HorizontalBrace { body, over } => NodeKind::HorizontalBrace {
                body: map(*body),
                over: *over,
            },
            NodeKind::Cancel { body, up, down } => NodeKind::Cancel {
                body: map(*body),
                up: *up,
                down: *down,
            },
            NodeKind::Styled { size, body } => NodeKind::Styled {
                size: *size,
                body: map(*body),
            },
            NodeKind::Variant { variant, body } => NodeKind::Variant {
                variant: *variant,
                body: map(*body),
            },
            NodeKind::Color { color, body } => NodeKind::Color {
                color: *color,
                body: map(*body),
            },
            NodeKind::Class {
                class,
                body,
                limits,
            } => NodeKind::Class {
                class: *class,
                body: map(*body),
                limits: *limits,
            },
            NodeKind::MathChoice {
                display,
                text,
                script,
                script_script,
            } => NodeKind::MathChoice {
                display: map(*display),
                text: map(*text),
                script: map(*script),
                script_script: map(*script_script),
            },
            NodeKind::UnderArrow { symbol, body } => NodeKind::UnderArrow {
                symbol: *symbol,
                body: map(*body),
            },
            NodeKind::Table {
                rows,
                columns,
                alignment,
                cell_style,
                column_rules,
                column_spacing,
            } => NodeKind::Table {
                rows: rows.iter().map(|id| map(*id)).collect(),
                columns: *columns,
                alignment: alignment.clone(),
                cell_style: *cell_style,
                column_rules: column_rules.clone(),
                column_spacing: *column_spacing,
            },
            NodeKind::TableRow { cells } => NodeKind::TableRow {
                cells: cells.iter().map(|id| map(*id)).collect(),
            },
            NodeKind::Error(text) => NodeKind::Error(text.clone()),
        }
    }
}

/// Why a structural edit was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditError {
    NoSuchNode,
    CannotRemoveRoot,
    /// The parent kind has no child at this slot, or the assignment would
    /// make a node contain itself.
    SlotNotEditable,
    IndexOutOfRange,
    /// Removing a required child would leave an invalid node.
    RequiredChild,
}

/// An arena of nodes with one root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    nodes: Vec<Node>,
    /// False for a slot on the free list; readers must not use a freed id.
    alive: Vec<bool>,
    free: Vec<NodeId>,
    root: NodeId,
}

impl Default for Model {
    fn default() -> Self {
        Model::new()
    }
}

impl Model {
    /// A model whose root is an empty row; `set_root` replaces it once the
    /// parser has built the real tree.
    pub fn new() -> Self {
        let mut model = Model {
            nodes: Vec::new(),
            alive: Vec::new(),
            free: Vec::new(),
            root: NodeId(0),
        };
        model.root = model.alloc(NodeKind::Row(Vec::new()), Span::default());
        model
    }

    /// Builds a model from `make`, which returns the root.
    pub fn build(make: impl FnOnce(&mut ModelBuilder) -> NodeId) -> (Self, NodeId) {
        let mut builder = ModelBuilder::new();
        let root = make(&mut builder);
        let model = builder.build(root);
        (model, root)
    }

    pub fn root(&self) -> NodeId {
        self.root
    }

    /// Replaces the root and frees every node the new tree no longer reaches,
    /// including the old root. A stale id is refused rather than making it the
    /// root of an empty model.
    pub fn set_root(&mut self, root: NodeId) -> Result<(), EditError> {
        if !self.is_alive(root) {
            return Err(EditError::NoSuchNode);
        }
        self.root = root;
        self.free_unreachable();
        Ok(())
    }

    /// Frees nodes outside the root's subtree so their ids can be reused.
    fn free_unreachable(&mut self) {
        let mut reachable = vec![false; self.nodes.len()];
        let mut stack = vec![self.root];
        while let Some(id) = stack.pop() {
            let index = id.index();
            if !self.is_alive(id) || reachable[index] {
                continue;
            }
            reachable[index] = true;
            stack.extend(self.kind(id).children().into_iter().map(|child| child.id));
        }
        for (index, reachable) in reachable.iter().enumerate() {
            if self.alive[index] && !*reachable {
                self.alive[index] = false;
                self.free.push(NodeId(index as u32));
            }
        }
    }

    pub fn alloc(&mut self, kind: NodeKind, span: Span) -> NodeId {
        self.alloc_with_origin(kind, span, Origin::Source)
    }

    pub fn alloc_with_origin(&mut self, kind: NodeKind, span: Span, origin: Origin) -> NodeId {
        let node = Node { kind, span, origin };
        if let Some(id) = self.free.pop() {
            self.nodes[id.index()] = node;
            self.alive[id.index()] = true;
            return id;
        }
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(node);
        self.alive.push(true);
        id
    }

    /// The node, or `None` when `id` was freed. A host that keeps ids across
    /// edits should use this rather than [`Model::node`].
    pub fn try_node(&self, id: NodeId) -> Option<&Node> {
        self.is_alive(id).then(|| &self.nodes[id.index()])
    }

    /// The node for an id the caller knows is live; reading a freed id is an
    /// invariant violation.
    pub fn node(&self, id: NodeId) -> &Node {
        match self.try_node(id) {
            Some(node) => node,
            None => panic!("read a freed node"),
        }
    }

    /// Whether `id` still names a node; a freed id may be reused later.
    pub fn is_alive(&self, id: NodeId) -> bool {
        self.alive.get(id.index()).copied().unwrap_or(false)
    }

    pub fn try_kind(&self, id: NodeId) -> Option<&NodeKind> {
        self.try_node(id).map(|node| &node.kind)
    }

    pub fn kind(&self, id: NodeId) -> &NodeKind {
        match self.try_kind(id) {
            Some(kind) => kind,
            None => panic!("read a freed node"),
        }
    }

    pub fn span(&self, id: NodeId) -> Span {
        self.node(id).span
    }

    pub fn set_span(&mut self, id: NodeId, span: Span) {
        self.nodes[id.index()].span = span;
    }

    pub fn set_origin(&mut self, id: NodeId, origin: Origin) {
        self.nodes[id.index()].origin = origin;
    }

    /// The number of live nodes.
    pub fn len(&self) -> usize {
        self.alive.iter().filter(|alive| **alive).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Arena slots, including freed ones; id indices stay below this.
    pub(crate) fn slots(&self) -> usize {
        self.nodes.len()
    }

    /// Every live node id, in allocation order.
    pub fn ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        (0..self.nodes.len() as u32)
            .map(NodeId)
            .filter(|id| self.is_alive(*id))
    }
}

/// Structural edits. Source text is never rewritten here: the writer exports
/// the model, and an external source change is a reparse.
impl Model {
    /// Inserts a new node into a list slot. Only rows, table rows and table
    /// cells have list slots; a single-child slot is reached through the child.
    pub fn insert(
        &mut self,
        parent: NodeId,
        slot: Slot,
        index: usize,
        kind: NodeKind,
        span: Span,
    ) -> Result<NodeId, EditError> {
        if !self.is_alive(parent) {
            return Err(EditError::NoSuchNode);
        }
        let length = self.list_length(parent, slot)?;
        if index > length {
            return Err(EditError::IndexOutOfRange);
        }
        let id = self.alloc(kind, span);
        self.splice_child(parent, slot, index, id);
        Ok(id)
    }

    /// Inserts an existing node into a list slot without allocating, for a
    /// host moving a copied or cut subtree.
    pub fn insert_existing(
        &mut self,
        parent: NodeId,
        slot: Slot,
        index: usize,
        child: NodeId,
    ) -> Result<(), EditError> {
        if !self.is_alive(parent) || !self.is_alive(child) {
            return Err(EditError::NoSuchNode);
        }
        if child == parent {
            return Err(EditError::SlotNotEditable);
        }
        if self.subtree_ids(child).contains(&parent) {
            return Err(EditError::SlotNotEditable);
        }
        let length = self.list_length(parent, slot)?;
        if index > length {
            return Err(EditError::IndexOutOfRange);
        }
        let duplicate = self
            .kind(parent)
            .children()
            .into_iter()
            .any(|slot_child| slot_child.slot == slot && slot_child.id == child);
        if duplicate {
            return Err(EditError::SlotNotEditable);
        }
        self.splice_child(parent, slot, index, child);
        Ok(())
    }

    fn list_length(&self, parent: NodeId, slot: Slot) -> Result<usize, EditError> {
        match (self.kind(parent), slot) {
            (NodeKind::Row(children), Slot::Body) => Ok(children.len()),
            (NodeKind::Table { rows, .. }, Slot::Row) => Ok(rows.len()),
            (NodeKind::TableRow { cells }, Slot::Cell) => Ok(cells.len()),
            _ => Err(EditError::SlotNotEditable),
        }
    }

    fn splice_child(&mut self, parent: NodeId, slot: Slot, index: usize, child: NodeId) {
        match (&mut self.nodes[parent.index()].kind, slot) {
            (NodeKind::Row(children), Slot::Body) => children.insert(index, child),
            (NodeKind::Table { rows, .. }, Slot::Row) => rows.insert(index, child),
            (NodeKind::TableRow { cells }, Slot::Cell) => cells.insert(index, child),
            _ => unreachable!("the slot was checked before the splice"),
        }
    }

    /// Detaches and frees `id` and its subtree. A freed id may be reused by a
    /// later allocation, so a host must drop it after this call.
    pub fn remove(&mut self, id: NodeId) -> Result<(), EditError> {
        if !self.is_alive(id) {
            return Err(EditError::NoSuchNode);
        }
        if id == self.root {
            return Err(EditError::CannotRemoveRoot);
        }
        let (parent, slot, index) = self.parent_of(id).ok_or(EditError::NoSuchNode)?;
        self.detach(parent, slot, index)?;
        self.free_subtree(id);
        Ok(())
    }

    /// Replaces a node's kind in place; children the new kind still refers to
    /// survive, and every other old subtree is freed.
    pub fn replace(&mut self, id: NodeId, kind: NodeKind) -> Result<(), EditError> {
        if !self.is_alive(id) {
            return Err(EditError::NoSuchNode);
        }
        let old_children: Vec<NodeId> =
            self.kind(id).children().into_iter().map(|c| c.id).collect();
        let mut kept = Vec::new();
        for child in kind.children() {
            if child.id == id {
                return Err(EditError::SlotNotEditable);
            }
            kept.extend(self.subtree_ids(child.id));
        }
        for child in old_children {
            self.free_subtree_except(child, &kept);
        }
        self.nodes[id.index()].kind = kind;
        Ok(())
    }

    /// Points the child slot `(parent, slot, index)` at `child`.
    ///
    /// The previous child survives only when `child`'s subtree still contains
    /// it; otherwise it is freed, so a host can wrap a node in one step. The
    /// call refuses a child that would contain its own parent.
    pub fn set_child(
        &mut self,
        parent: NodeId,
        slot: Slot,
        index: usize,
        child: NodeId,
    ) -> Result<(), EditError> {
        if !self.is_alive(parent) || !self.is_alive(child) {
            return Err(EditError::NoSuchNode);
        }
        if child == parent {
            return Err(EditError::SlotNotEditable);
        }
        let kept = self.subtree_ids(child);
        if kept.contains(&parent) {
            return Err(EditError::SlotNotEditable);
        }
        let old = self.assign_child(parent, slot, index, child)?;
        if let Some(old) = old
            && !kept.contains(&old)
        {
            self.free_subtree_except(old, &kept);
        }
        Ok(())
    }

    /// Copies `source`'s subtree into this model, allocating every node again.
    ///
    /// Spans point into the source that produced them, so the copy carries the
    /// caller's span and origin instead; a parsed snippet becomes synthetic
    /// content at the caret.
    pub fn copy_subtree(
        &mut self,
        source: &Model,
        id: NodeId,
        span: Span,
        origin: Origin,
    ) -> NodeId {
        let kind = source
            .kind(id)
            .map_children(|child| self.copy_subtree(source, child, span, origin));
        self.alloc_with_origin(kind, span, origin)
    }

    fn assign_child(
        &mut self,
        parent: NodeId,
        slot: Slot,
        index: usize,
        child: NodeId,
    ) -> Result<Option<NodeId>, EditError> {
        let listed = matches!(
            (&self.nodes[parent.index()].kind, slot),
            (NodeKind::Row(_), Slot::Body)
                | (NodeKind::TableRow { .. }, Slot::Cell)
                | (NodeKind::Table { .. }, Slot::Row)
        );
        if !listed && index != 0 {
            return Err(EditError::IndexOutOfRange);
        }
        let old: Option<NodeId> = match &mut self.nodes[parent.index()].kind {
            NodeKind::Row(children) => match slot {
                Slot::Body => Some(replace_at(children, index, child)?),
                _ => return Err(EditError::SlotNotEditable),
            },
            NodeKind::TableRow { cells } => match slot {
                Slot::Cell => Some(replace_at(cells, index, child)?),
                _ => return Err(EditError::SlotNotEditable),
            },
            NodeKind::Table { rows, .. } => match slot {
                Slot::Row => Some(replace_at(rows, index, child)?),
                _ => return Err(EditError::SlotNotEditable),
            },
            NodeKind::Scripts { base, sup, sub } => match slot {
                Slot::Base => Some(std::mem::replace(base, child)),
                Slot::Superscript => {
                    let old = sup.take();
                    *sup = Some(child);
                    old
                }
                Slot::Subscript => {
                    let old = sub.take();
                    *sub = Some(child);
                    old
                }
                _ => return Err(EditError::SlotNotEditable),
            },
            NodeKind::Fraction {
                numerator,
                denominator,
                ..
            } => match slot {
                Slot::Numerator => Some(std::mem::replace(numerator, child)),
                Slot::Denominator => Some(std::mem::replace(denominator, child)),
                _ => return Err(EditError::SlotNotEditable),
            },
            NodeKind::Radical { body, degree } => match slot {
                Slot::Body => Some(std::mem::replace(body, child)),
                Slot::Degree => {
                    let old = degree.take();
                    *degree = Some(child);
                    old
                }
                _ => return Err(EditError::SlotNotEditable),
            },
            NodeKind::Accent { base, .. } => match slot {
                Slot::Base => Some(std::mem::replace(base, child)),
                _ => return Err(EditError::SlotNotEditable),
            },
            NodeKind::Delimited { body, .. }
            | NodeKind::Overline { body }
            | NodeKind::Underline { body }
            | NodeKind::Phantom { body, .. }
            | NodeKind::Smash { body, .. }
            | NodeKind::ColorBox { body, .. }
            | NodeKind::Boxed { body }
            | NodeKind::Cancel { body, .. }
            | NodeKind::HorizontalBrace { body, .. }
            | NodeKind::Styled { body, .. }
            | NodeKind::Variant { body, .. }
            | NodeKind::Color { body, .. }
            | NodeKind::Class { body, .. }
            | NodeKind::UnderArrow { body, .. } => match slot {
                Slot::Body => Some(std::mem::replace(body, child)),
                _ => return Err(EditError::SlotNotEditable),
            },
            NodeKind::OverUnder { base, over, under } => match slot {
                Slot::Base => Some(std::mem::replace(base, child)),
                Slot::Over => {
                    let old = over.take();
                    *over = Some(child);
                    old
                }
                Slot::Under => {
                    let old = under.take();
                    *under = Some(child);
                    old
                }
                _ => return Err(EditError::SlotNotEditable),
            },
            NodeKind::Arrow { over, under, .. } => match slot {
                Slot::Over => {
                    let old = over.take();
                    *over = Some(child);
                    old
                }
                Slot::Under => {
                    let old = under.take();
                    *under = Some(child);
                    old
                }
                _ => return Err(EditError::SlotNotEditable),
            },
            NodeKind::MathChoice {
                display,
                text,
                script,
                script_script,
            } => match slot {
                Slot::Choice(Size::Display) => Some(std::mem::replace(display, child)),
                Slot::Choice(Size::Text) => Some(std::mem::replace(text, child)),
                Slot::Choice(Size::Script) => Some(std::mem::replace(script, child)),
                Slot::Choice(Size::ScriptScript) => Some(std::mem::replace(script_script, child)),
                _ => return Err(EditError::SlotNotEditable),
            },
            _ => return Err(EditError::SlotNotEditable),
        };
        Ok(old)
    }

    /// The parent, slot and slot-relative index of `id`, if any.
    fn parent_of(&self, id: NodeId) -> Option<(NodeId, Slot, usize)> {
        for parent in self.ids() {
            let mut counts: Vec<(Slot, usize)> = Vec::new();
            for child in self.kind(parent).children() {
                let index = counts
                    .iter()
                    .find(|(slot, _)| *slot == child.slot)
                    .map_or(0, |(_, count)| *count);
                if child.id == id {
                    return Some((parent, child.slot, index));
                }
                match counts.iter_mut().find(|(slot, _)| *slot == child.slot) {
                    Some((_, count)) => *count += 1,
                    None => counts.push((child.slot, 1)),
                }
            }
        }
        None
    }

    fn detach(&mut self, parent: NodeId, slot: Slot, index: usize) -> Result<(), EditError> {
        let kind = &mut self.nodes[parent.index()].kind;
        match (kind, slot) {
            (NodeKind::Row(children), Slot::Body) => {
                if index >= children.len() {
                    return Err(EditError::IndexOutOfRange);
                }
                children.remove(index);
            }
            (NodeKind::Table { rows, .. }, Slot::Row) => {
                if index >= rows.len() {
                    return Err(EditError::IndexOutOfRange);
                }
                rows.remove(index);
            }
            (NodeKind::TableRow { cells }, Slot::Cell) => {
                if index >= cells.len() {
                    return Err(EditError::IndexOutOfRange);
                }
                cells.remove(index);
            }
            (NodeKind::Scripts { sup, .. }, Slot::Superscript) => *sup = None,
            (NodeKind::Scripts { sub, .. }, Slot::Subscript) => *sub = None,
            (NodeKind::Radical { degree, .. }, Slot::Degree) => *degree = None,
            (NodeKind::OverUnder { over, .. }, Slot::Over) => *over = None,
            (NodeKind::OverUnder { under, .. }, Slot::Under) => *under = None,
            (NodeKind::Arrow { over, .. }, Slot::Over) => *over = None,
            (NodeKind::Arrow { under, .. }, Slot::Under) => *under = None,
            (_, Slot::Numerator | Slot::Denominator | Slot::Base | Slot::Body) => {
                return Err(EditError::RequiredChild);
            }
            _ => return Err(EditError::SlotNotEditable),
        }
        Ok(())
    }

    fn subtree_ids(&self, id: NodeId) -> Vec<NodeId> {
        if !self.is_alive(id) {
            return Vec::new();
        }
        let mut ids = vec![id];
        for child in self.kind(id).children() {
            ids.extend(self.subtree_ids(child.id));
        }
        ids
    }

    /// Frees `id`'s subtree except the nodes the replacement still refers to.
    fn free_subtree_except(&mut self, id: NodeId, kept: &[NodeId]) {
        if !self.is_alive(id) || kept.contains(&id) {
            return;
        }
        let children: Vec<NodeId> = self.kind(id).children().into_iter().map(|c| c.id).collect();
        for child in children {
            self.free_subtree_except(child, kept);
        }
        self.alive[id.index()] = false;
        self.free.push(id);
    }

    fn free_subtree(&mut self, id: NodeId) {
        if !self.is_alive(id) {
            return;
        }
        let children: Vec<NodeId> = self.kind(id).children().into_iter().map(|c| c.id).collect();
        for child in children {
            self.free_subtree(child);
        }
        self.alive[id.index()] = false;
        self.free.push(id);
    }
}

fn replace_at(children: &mut [NodeId], index: usize, child: NodeId) -> Result<NodeId, EditError> {
    let slot = children.get_mut(index).ok_or(EditError::IndexOutOfRange)?;
    Ok(std::mem::replace(slot, child))
}

/// A model under construction. The parser allocates into it and fixes the
/// root at the end, so a `Model` always has one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelBuilder {
    nodes: Vec<Node>,
}

impl ModelBuilder {
    pub fn new() -> Self {
        ModelBuilder { nodes: Vec::new() }
    }

    pub fn alloc(&mut self, kind: NodeKind, span: Span) -> NodeId {
        self.alloc_with_origin(kind, span, Origin::Source)
    }

    pub fn alloc_with_origin(&mut self, kind: NodeKind, span: Span, origin: Origin) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(Node { kind, span, origin });
        id
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.index()]
    }

    pub fn kind(&self, id: NodeId) -> &NodeKind {
        &self.node(id).kind
    }

    pub fn span(&self, id: NodeId) -> Span {
        self.node(id).span
    }

    pub fn set_span(&mut self, id: NodeId, span: Span) {
        self.nodes[id.index()].span = span;
    }

    pub fn set_origin(&mut self, id: NodeId, origin: Origin) {
        self.nodes[id.index()].origin = origin;
    }

    /// Replaces a node's kind in place, for a suffix that changes only the
    /// node's own fields. Unlike [`Model::replace`], the builder never frees.
    pub fn replace(&mut self, id: NodeId, kind: NodeKind) {
        self.nodes[id.index()].kind = kind;
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn build(self, root: NodeId) -> Model {
        let mut alive = vec![false; self.nodes.len()];
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            let Some(mark) = alive.get_mut(id.index()) else {
                continue;
            };
            if *mark {
                continue;
            }
            *mark = true;
            pending.extend(
                self.nodes[id.index()]
                    .kind
                    .children()
                    .into_iter()
                    .map(|child| child.id),
            );
        }
        // The parser abandons nodes when it unwraps a singleton group; a node
        // unreachable from the root must not stay alive, or `parent_of` can
        // resolve it as a second parent and detach a still-referenced child.
        let free = (0..self.nodes.len() as u32)
            .map(NodeId)
            .filter(|id| !alive[id.index()])
            .collect();
        Model {
            nodes: self.nodes,
            alive,
            free,
            root,
        }
    }
}

/// Untimed constructors for tests, demos and the parser's fixed leaves.
///
/// The parser allocates nodes with an explicit span instead; these exist so a
/// caller building a tree by hand need not repeat `alloc(kind, Span::default())`.
impl ModelBuilder {
    pub fn atom(&mut self, atom: Atom) -> NodeId {
        self.alloc(NodeKind::Atom(atom), Span::default())
    }

    pub fn symbol(&mut self, symbol: char) -> NodeId {
        self.atom(Atom::new(symbol))
    }

    pub fn row(&mut self, children: Vec<NodeId>) -> NodeId {
        self.alloc(NodeKind::Row(children), Span::default())
    }

    pub fn scripts(&mut self, base: NodeId, sup: Option<NodeId>, sub: Option<NodeId>) -> NodeId {
        self.alloc(NodeKind::Scripts { base, sup, sub }, Span::default())
    }

    pub fn fraction(&mut self, numerator: NodeId, denominator: NodeId, bar: bool) -> NodeId {
        let rule = if bar {
            FractionRule::Default
        } else {
            FractionRule::None
        };
        self.fraction_styled(numerator, denominator, rule, None)
    }

    pub fn fraction_styled(
        &mut self,
        numerator: NodeId,
        denominator: NodeId,
        rule: FractionRule,
        style: Option<Size>,
    ) -> NodeId {
        self.alloc(
            NodeKind::Fraction {
                numerator,
                denominator,
                rule,
                style,
            },
            Span::default(),
        )
    }

    pub fn radical(&mut self, body: NodeId, degree: Option<NodeId>) -> NodeId {
        self.alloc(NodeKind::Radical { body, degree }, Span::default())
    }

    pub fn delimited(&mut self, left: Option<char>, body: NodeId, right: Option<char>) -> NodeId {
        self.alloc(NodeKind::Delimited { left, body, right }, Span::default())
    }

    /// `limits` is the symbol table's default placement in display style.
    pub fn large_op(&mut self, symbol: char, limits: bool) -> NodeId {
        self.alloc(
            NodeKind::LargeOp {
                symbol,
                limits: limits.into(),
            },
            Span::default(),
        )
    }

    pub fn accent(&mut self, base: NodeId, symbol: char) -> NodeId {
        self.alloc(NodeKind::Accent { base, symbol }, Span::default())
    }

    pub fn text(&mut self, text: impl Into<String>) -> NodeId {
        self.alloc(NodeKind::Text(text.into()), Span::default())
    }

    pub fn composition(&mut self, text: impl Into<String>) -> NodeId {
        self.alloc_with_origin(
            NodeKind::Composition(text.into()),
            Span::default(),
            Origin::Synthetic,
        )
    }

    /// `limits` is the operator's default placement in display style.
    pub fn function(&mut self, name: impl Into<String>, limits: bool) -> NodeId {
        self.alloc(
            NodeKind::Function {
                name: name.into(),
                limits: limits.into(),
            },
            Span::default(),
        )
    }

    pub fn space(&mut self, kind: SpaceKind) -> NodeId {
        self.alloc(NodeKind::Space(kind), Span::default())
    }

    pub fn hspace(&mut self, length: Length) -> NodeId {
        self.alloc(NodeKind::Hspace(length), Span::default())
    }

    pub fn overline(&mut self, body: NodeId) -> NodeId {
        self.alloc(NodeKind::Overline { body }, Span::default())
    }

    pub fn underline(&mut self, body: NodeId) -> NodeId {
        self.alloc(NodeKind::Underline { body }, Span::default())
    }

    pub fn middle(&mut self, symbol: char) -> NodeId {
        self.alloc(NodeKind::Middle { symbol }, Span::default())
    }

    pub fn sized_delimiter(
        &mut self,
        symbol: char,
        size: DelimiterSize,
        class: AtomClass,
    ) -> NodeId {
        self.alloc(
            NodeKind::SizedDelimiter {
                symbol,
                size,
                class,
            },
            Span::default(),
        )
    }

    pub fn phantom(&mut self, body: NodeId, kind: PhantomKind) -> NodeId {
        self.alloc(NodeKind::Phantom { body, kind }, Span::default())
    }

    pub fn smash(&mut self, body: NodeId, top: bool, bottom: bool) -> NodeId {
        self.alloc(NodeKind::Smash { body, top, bottom }, Span::default())
    }

    pub fn boxed(&mut self, body: NodeId) -> NodeId {
        self.alloc(NodeKind::Boxed { body }, Span::default())
    }

    pub fn color_box(&mut self, background: Color, border: Option<Color>, body: NodeId) -> NodeId {
        self.alloc(
            NodeKind::ColorBox {
                background,
                border,
                body,
            },
            Span::default(),
        )
    }

    pub fn overset(&mut self, base: NodeId, over: Option<NodeId>, under: Option<NodeId>) -> NodeId {
        self.alloc(NodeKind::OverUnder { base, over, under }, Span::default())
    }

    pub fn arrow(&mut self, symbol: char, over: Option<NodeId>, under: Option<NodeId>) -> NodeId {
        self.alloc(
            NodeKind::Arrow {
                symbol,
                over,
                under,
            },
            Span::default(),
        )
    }

    pub fn horizontal_brace(&mut self, body: NodeId, over: bool) -> NodeId {
        self.alloc(NodeKind::HorizontalBrace { body, over }, Span::default())
    }

    pub fn cancel(&mut self, body: NodeId, up: bool, down: bool) -> NodeId {
        self.alloc(NodeKind::Cancel { body, up, down }, Span::default())
    }

    pub fn styled(&mut self, size: Size, body: NodeId) -> NodeId {
        self.alloc(NodeKind::Styled { size, body }, Span::default())
    }

    pub fn variant(&mut self, variant: Variant, body: NodeId) -> NodeId {
        self.alloc(NodeKind::Variant { variant, body }, Span::default())
    }

    pub fn color(&mut self, color: Color, body: NodeId) -> NodeId {
        self.alloc(NodeKind::Color { color, body }, Span::default())
    }

    /// A class wrapper; only an `Op` class reacts to `limits`.
    pub fn classed(&mut self, class: AtomClass, body: NodeId, limits: Limits) -> NodeId {
        self.alloc(
            NodeKind::Class {
                class,
                body,
                limits,
            },
            Span::default(),
        )
    }

    /// The four branches of `\mathchoice`, in style order.
    pub fn math_choice(
        &mut self,
        display: NodeId,
        text: NodeId,
        script: NodeId,
        script_script: NodeId,
    ) -> NodeId {
        self.alloc(
            NodeKind::MathChoice {
                display,
                text,
                script,
                script_script,
            },
            Span::default(),
        )
    }

    pub fn table(
        &mut self,
        rows: Vec<NodeId>,
        columns: usize,
        alignment: Vec<Alignment>,
    ) -> NodeId {
        self.alloc(
            NodeKind::Table {
                rows,
                columns,
                alignment,
                cell_style: None,
                column_rules: Vec::new(),
                column_spacing: ColumnSpacing::Spaced,
            },
            Span::default(),
        )
    }

    pub fn table_row(&mut self, cells: Vec<NodeId>) -> NodeId {
        self.alloc(NodeKind::TableRow { cells }, Span::default())
    }

    pub fn error(&mut self, text: impl Into<String>) -> NodeId {
        self.alloc(NodeKind::Error(text.into()), Span::default())
    }
}
