//! Host-side editing over the math model.
//!
//! Structure is the document and LaTeX is an export: keystrokes insert, remove
//! and replace nodes, then the whole formula is laid out again. A host owns
//! painting, input translation and the text shaper; this crate owns what an
//! edit does to the model, where the caret lands, and what undo restores.

use math_core::{
    Atom, CaretPath, CaretRect, Diagnostic, EditError, Geometry, Layout, MathFont, Model, NodeId,
    NodeKind, Origin, Point, Rect, Slot, Span, Step, Style, TextShaper, layout_with_geometry,
    layout_with_shaper_and_geometry,
};
use math_syntax::{
    CommandShape, Document, Spellings, command_shape, command_template, delimiter_symbol, parse,
    to_latex_with_spans_and_spellings, to_latex_with_spellings,
};
use std::sync::Arc;

/// The edits a host forwards; modifier state is a separate argument because
/// only Shift changes what an editor command means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Character(char),
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Escape,
}

const UNDO_LIMIT: usize = 256;

#[derive(Clone, Debug)]
struct Snapshot {
    model: Model,
    spellings: Spellings,
    caret: CaretPath,
    anchor: Option<CaretPath>,
}

struct Preedit {
    node: NodeId,
    row: NodeId,
    index: usize,
    /// The document before the composition appeared; committing pushes this
    /// so undo removes the committed content and never shows the preedit.
    before: Snapshot,
}

/// A formula being edited: model, layout, caret, selection and history.
pub struct Editor {
    model: Model,
    spellings: Spellings,
    layout: Layout,
    geometry: Geometry,
    parse_diagnostics: Vec<Diagnostic>,
    diagnostics: Vec<Diagnostic>,
    caret: CaretPath,
    anchor: Option<CaretPath>,
    size: f32,
    style: Style,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    shaper: Option<Arc<dyn TextShaper>>,
    preedit: Option<Preedit>,
    command: Option<String>,
    /// The name after a second `\`, as in `\left\langle`; `command` keeps
    /// the first name until both are resolved together.
    command_argument: Option<String>,
    /// The `Composition` node that shows the command being typed, and the
    /// document before it appeared. Cancelling restores that snapshot.
    command_node: Option<NodeId>,
    command_before: Option<Snapshot>,
    /// Where the IME's current contribution starts inside `command`. An IME
    /// composing `f`, `fr`, … replaces that tail instead of appending it, and
    /// a commit replaces it with the final text.
    command_ime_start: Option<usize>,
    message: Option<String>,
}

impl Editor {
    /// Parses `source`, lays it out and puts the caret at the end.
    pub fn new(source: &str, font: &dyn MathFont, size: f32, style: Style) -> Self {
        let document = parse(source);
        let (layout, geometry, layout_diagnostics) =
            layout_with_geometry(font, &document.model, document.root(), size, style);
        let mut diagnostics = document.diagnostics.clone();
        diagnostics.extend(layout_diagnostics);
        let caret = geometry
            .carets()
            .last()
            .map(|(path, _)| path.clone())
            .unwrap_or_default();
        Editor {
            model: document.model,
            spellings: document.spellings,
            layout,
            geometry,
            parse_diagnostics: document.diagnostics,
            diagnostics,
            caret,
            anchor: None,
            size,
            style,
            undo: Vec::new(),
            redo: Vec::new(),
            shaper: None,
            preedit: None,
            command: None,
            command_argument: None,
            command_node: None,
            command_before: None,
            command_ime_start: None,
            message: None,
        }
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn geometry(&self) -> &Geometry {
        &self.geometry
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn source(&self) -> String {
        to_latex_with_spellings(&self.model, self.model.root(), &self.spellings)
    }

    /// Byte range of `node` in `source()`, or `None` if it is no longer alive.
    ///
    /// Every layout refreshes these offsets from the exported text, so they
    /// describe the current export rather than the document the editor parsed.
    pub fn source_span(&self, node: NodeId) -> Option<Span> {
        self.model.is_alive(node).then(|| self.model.span(node))
    }

    fn normalize_spans(&mut self) {
        let (_, spans) =
            to_latex_with_spans_and_spellings(&self.model, self.model.root(), &self.spellings);
        for (node, span) in spans {
            self.model.set_span(node, span);
        }
    }

    pub fn caret(&self) -> &CaretPath {
        &self.caret
    }

    pub fn caret_rect(&self) -> Option<CaretRect> {
        self.geometry.caret(&self.caret)
    }

    /// Boxes of required slots that hold an empty row.
    ///
    /// The editing layout reserves each box, so it takes the same space as the
    /// content that will replace it; a host only paints it.
    pub fn placeholders(&self) -> Vec<(NodeId, Rect)> {
        self.geometry
            .empty_slots()
            .map(|slot| (slot.node, slot.rect))
            .collect()
    }

    pub fn anchor(&self) -> Option<&CaretPath> {
        self.anchor.as_ref()
    }

    /// Rectangles covering the selected content, one per line.
    pub fn selection(&self) -> Vec<Rect> {
        match &self.anchor {
            Some(anchor) if anchor != &self.caret => self.geometry.selection(anchor, &self.caret),
            _ => Vec::new(),
        }
    }

    /// A pending `\command` name, which the host may show.
    pub fn command(&self) -> Option<&str> {
        self.command.as_deref()
    }

    /// The live IME preedit, if any.
    pub fn preedit(&self) -> Option<&str> {
        let preedit = self.preedit.as_ref()?;
        match self.model.kind(preedit.node) {
            NodeKind::Composition(text) => Some(text),
            _ => None,
        }
    }

    /// The last refused command, for a host status line.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub fn size(&self) -> f32 {
        self.size
    }

    pub fn style(&self) -> Style {
        self.style
    }

    /// Replaces the document from source text. This is the external change
    /// path: ids are new, history is dropped, and the caret keeps its reading
    /// position when it can.
    pub fn set_source(&mut self, source: &str, font: &dyn MathFont) {
        if self.command.is_some() {
            self.cancel_command(font);
        }
        let ordinal = self.caret_index().unwrap_or(usize::MAX);
        let document = parse(source);
        self.model = document.model;
        self.spellings = document.spellings;
        self.parse_diagnostics = document.diagnostics;
        self.undo.clear();
        self.redo.clear();
        self.preedit = None;
        self.command = None;
        self.command_argument = None;
        self.command_node = None;
        self.command_before = None;
        self.command_ime_start = None;
        self.anchor = None;
        self.message = None;
        self.relayout(font);
        let count = self.geometry.carets().count();
        let target = ordinal.min(count.saturating_sub(1));
        self.caret = self.stop_path(target).unwrap_or_default();
    }

    pub fn set_size(&mut self, size: f32, font: &dyn MathFont) {
        self.size = size;
        self.relayout(font);
    }

    pub fn set_style(&mut self, style: Style, font: &dyn MathFont) {
        self.style = style;
        self.relayout(font);
    }

    /// Installs a host text shaper and lays the formula out again.
    pub fn set_shaper(&mut self, shaper: Arc<dyn TextShaper>, font: &dyn MathFont) {
        self.shaper = Some(shaper);
        self.relayout(font);
    }

    /// Lays the current model out again after a font or size change.
    pub fn relayout(&mut self, font: &dyn MathFont) {
        self.normalize_spans();
        let (layout, geometry, layout_diagnostics) = match &self.shaper {
            Some(shaper) => layout_with_shaper_and_geometry(
                font,
                &**shaper,
                &self.model,
                self.model.root(),
                self.size,
                self.style,
            ),
            None => {
                layout_with_geometry(font, &self.model, self.model.root(), self.size, self.style)
            }
        };
        self.layout = layout;
        self.geometry = geometry;
        self.diagnostics.clear();
        self.diagnostics
            .extend(self.parse_diagnostics.iter().cloned());
        self.diagnostics.extend(layout_diagnostics);
        self.repair_caret();
    }

    fn repair_caret(&mut self) {
        if self.geometry.caret(&self.caret).is_none() {
            self.caret = self
                .geometry
                .carets()
                .last()
                .map(|(path, _)| path.clone())
                .unwrap_or_default();
        }
        if let Some(anchor) = &self.anchor
            && self.geometry.caret(anchor).is_none()
        {
            self.anchor = None;
        }
    }

    /// Moves the caret to the position nearest `point`; `extend` keeps the
    /// existing anchor and starts a selection when there is none.
    pub fn click(&mut self, point: Point, extend: bool) -> bool {
        let Some(path) = self.geometry.hit_test(point) else {
            return false;
        };
        if extend {
            if self.anchor.is_none() {
                self.anchor = Some(self.caret.clone());
            }
        } else {
            self.anchor = None;
        }
        let changed = path != self.caret || !extend;
        self.caret = path;
        changed
    }

    /// Selects the whole formula, leaving the caret at its end.
    pub fn select_all(&mut self) -> bool {
        let count = self.geometry.carets().count();
        if count == 0 {
            return false;
        }
        let (Some(first), Some(last)) = (self.stop_path(0), self.stop_path(count - 1)) else {
            return false;
        };
        self.anchor = Some(first);
        self.caret = last;
        true
    }

    /// LaTeX for the selected content, or `None` when nothing is selected.
    ///
    /// Only nodes wholly inside the range are copied; a range that cuts
    /// through a structure copies the covered content inside it. This keeps
    /// the result valid LaTeX that `insert_latex` can parse again.
    pub fn selection_latex(&self) -> Option<String> {
        let (low, high) = self.selection_range()?;
        let mut nodes = Vec::new();
        self.collect_covered(self.model.root(), low, high, &mut nodes);
        if nodes.is_empty() {
            return None;
        }
        let mut text = String::new();
        for node in nodes {
            text.push_str(&to_latex_with_spellings(&self.model, node, &self.spellings));
        }
        Some(text)
    }

    /// Copies the selection and removes it in one undo step.
    pub fn cut_selection(&mut self, font: &dyn MathFont) -> Option<String> {
        let text = self.selection_latex()?;
        self.begin();
        self.delete_selection_raw(font);
        self.end();
        Some(text)
    }

    fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.anchor.as_ref()?;
        if *anchor == self.caret {
            return None;
        }
        let anchor = self.stop_index(anchor)?;
        let focus = self.stop_index(&self.caret)?;
        Some((anchor.min(focus), anchor.max(focus)))
    }

    fn collect_covered(&self, parent: NodeId, low: usize, high: usize, out: &mut Vec<NodeId>) {
        for (node, slot, index) in self.indexed_children(parent) {
            let Some((start, end)) = self.node_interval(parent, slot, index) else {
                continue;
            };
            if end <= low || start >= high {
                continue;
            }
            if start >= low && end <= high {
                out.push(node);
            } else if !self.model.kind(node).children().is_empty() {
                self.collect_covered(node, low, high, out);
            }
        }
    }

    /// Wraps the preceding child in a superscript and puts the caret in it.
    pub fn insert_superscript(&mut self, font: &dyn MathFont) -> bool {
        if self.command.is_some() {
            self.cancel_command(font);
        }
        self.insert_script(Slot::Superscript, font)
    }

    /// Wraps the preceding child in a subscript and puts the caret in it.
    pub fn insert_subscript(&mut self, font: &dyn MathFont) -> bool {
        if self.command.is_some() {
            self.cancel_command(font);
        }
        self.insert_script(Slot::Subscript, font)
    }

    /// Applies one host key. Returns whether the display changed.
    pub fn key(&mut self, key: Key, shift: bool, font: &dyn MathFont) -> bool {
        self.message = None;
        if self.command.is_some() {
            return self.command_key(key, shift, font);
        }
        if self.preedit.is_some() {
            if key == Key::Escape {
                self.cancel_preedit(font);
                return true;
            }
            let text = self.preedit().unwrap_or_default().to_owned();
            self.commit_preedit(&text, font);
        }
        match key {
            Key::Character('\\') => self.begin_command(font),
            Key::Character('^') => self.insert_script(Slot::Superscript, font),
            Key::Character('_') => self.insert_script(Slot::Subscript, font),
            Key::Character('{') | Key::Character('}') => false,
            Key::Character(symbol) if symbol.is_whitespace() => false,
            Key::Character(symbol) => self.insert_atom(symbol, font),
            Key::Backspace => {
                self.begin();
                let selected = self.delete_selection_raw(font);
                let deleted = if selected {
                    false
                } else {
                    self.delete_before_raw(font)
                };
                self.end();
                selected || deleted
            }
            Key::Delete => {
                self.begin();
                let selected = self.delete_selection_raw(font);
                let deleted = if selected {
                    false
                } else {
                    self.delete_after_raw(font)
                };
                self.end();
                selected || deleted
            }
            Key::Left => self.move_horizontal(-1, shift),
            Key::Right => self.move_horizontal(1, shift),
            Key::Up => self.move_vertical(-1, shift, font),
            Key::Down => self.move_vertical(1, shift, font),
            Key::Home => self.move_to_row_edge(false, shift),
            Key::End => self.move_to_row_edge(true, shift),
            Key::Escape => self.anchor.take().is_some(),
        }
    }

    /// Inserts a LaTeX snippet at the caret as structural nodes, reusing the
    /// parser instead of duplicating command knowledge.
    pub fn insert_latex(&mut self, source: &str, font: &dyn MathFont) -> bool {
        self.message = None;
        if self.command.is_some() {
            self.cancel_command(font);
        }
        self.begin();
        self.delete_selection_raw(font);
        let Some((row, index)) = self.insertion_row() else {
            self.end();
            self.message = Some("cannot insert here".to_owned());
            return false;
        };
        let inserted = self.insert_parsed_at(row, index, source, font);
        self.end();
        inserted
    }

    pub fn undo(&mut self, font: &dyn MathFont) -> bool {
        if self.command.is_some() {
            self.cancel_command(font);
        }
        if self.preedit.is_some() {
            self.cancel_preedit(font);
        }
        let Some(snapshot) = self.undo.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.redo.push(current);
        self.restore(snapshot);
        self.relayout(font);
        true
    }

    pub fn redo(&mut self, font: &dyn MathFont) -> bool {
        if self.command.is_some() {
            self.cancel_command(font);
        }
        if self.preedit.is_some() {
            self.cancel_preedit(font);
        }
        let Some(snapshot) = self.redo.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.undo.push(current);
        self.restore(snapshot);
        self.relayout(font);
        true
    }

    /// Updates the live composition, replacing it with `text`; empty text
    /// dismisses it.
    pub fn set_preedit(&mut self, text: &str, font: &dyn MathFont) -> bool {
        self.message = None;
        if self.command.is_some() && is_command_text(text) {
            return self.absorb_command_ime(text, font);
        }
        if self.command.is_some() {
            self.cancel_command(font);
        }
        if text.is_empty() {
            return self.cancel_preedit(font);
        }
        if self.preedit.is_some() {
            let Some(preedit) = &self.preedit else {
                return false;
            };
            let node = preedit.node;
            let row = preedit.row;
            let index = preedit.index;
            if self
                .replace_kind(node, NodeKind::Composition(text.to_owned()))
                .is_err()
            {
                return false;
            }
            self.model.set_origin(node, Origin::Synthetic);
            self.caret = self
                .boundary_path(row, Slot::Body, index + 1)
                .unwrap_or_else(|| self.caret.clone());
            self.anchor = None;
            self.relayout(font);
            return true;
        }
        // The composition must not enter history, so no checkpoint is taken
        // here; the snapshot before every preedit mutation is what a commit
        // pushes and a cancel restores.
        let before = self.snapshot();
        self.delete_selection_raw(font);
        let Some((row, index)) = self.insertion_row() else {
            self.restore(before);
            self.relayout(font);
            return false;
        };
        let node = match self.model.insert(
            row,
            Slot::Body,
            index,
            NodeKind::Composition(text.to_owned()),
            Span::default(),
        ) {
            Ok(node) => node,
            Err(_) => {
                self.restore(before);
                self.relayout(font);
                return false;
            }
        };
        self.model.set_origin(node, Origin::Synthetic);
        self.caret = self
            .boundary_path(row, Slot::Body, index + 1)
            .unwrap_or_else(|| self.caret.clone());
        self.anchor = None;
        self.preedit = Some(Preedit {
            node,
            row,
            index,
            before,
        });
        self.relayout(font);
        true
    }

    /// Ends the composition, entering `text` as math in one edit. The preedit
    /// itself never enters the undo history.
    pub fn commit_preedit(&mut self, text: &str, font: &dyn MathFont) -> bool {
        self.message = None;
        if self.command.is_some() && is_command_text(text) {
            let absorbed = self.absorb_command_ime(text, font);
            // A commit ends the composition; the next letter is typed, not
            // another preedit update to replace.
            self.command_ime_start = None;
            return absorbed;
        }
        if self.command.is_some() {
            self.cancel_command(font);
        }
        let Some(preedit) = self.preedit.take() else {
            return self.insert_text(text, font);
        };
        if text.is_empty() {
            self.restore(preedit.before);
            self.relayout(font);
            return true;
        }
        let _ = self.remove_node(preedit.node);
        self.caret = self
            .boundary_path(preedit.row, Slot::Body, preedit.index)
            .unwrap_or_else(|| self.caret.clone());
        self.anchor = None;
        self.undo.push(preedit.before);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.relayout(font);
        let inserted = self.insert_parsed_at(preedit.row, preedit.index, text, font);
        if !inserted
            && self
                .undo
                .last()
                .is_some_and(|snapshot| snapshot.model == self.model)
        {
            self.undo.pop();
        }
        true
    }

    pub fn cancel_preedit(&mut self, font: &dyn MathFont) -> bool {
        let Some(preedit) = self.preedit.take() else {
            return false;
        };
        self.restore(preedit.before);
        self.relayout(font);
        true
    }

    fn insert_text(&mut self, text: &str, font: &dyn MathFont) -> bool {
        if text.is_empty() {
            return false;
        }
        self.begin();
        self.delete_selection_raw(font);
        let Some((row, index)) = self.insertion_row() else {
            self.end();
            return false;
        };
        let inserted = self.insert_parsed_at(row, index, text, font);
        self.end();
        inserted
    }

    fn command_key(&mut self, key: Key, shift: bool, font: &dyn MathFont) -> bool {
        if self.command_argument.is_some() {
            return self.command_argument_key(key, shift, font);
        }
        match key {
            // A second `\` names a delimiter or relation command, as in
            // `\left\langle` and `\not\in`; letters arrive in the nested
            // buffer instead of extending the first name.
            Key::Character('\\')
                if self.command.as_deref().is_some_and(|name| {
                    matches!(
                        command_shape(name),
                        CommandShape::Delimiter | CommandShape::SymbolArgument
                    )
                }) =>
            {
                self.command_argument = Some(String::new());
                self.command_ime_start = None;
                self.update_command(font);
                true
            }
            Key::Character(symbol) if is_command_letter(symbol) => {
                if let Some(command) = &mut self.command {
                    command.push(symbol);
                }
                self.command_ime_start = None;
                self.update_command(font);
                true
            }
            Key::Backspace => {
                let empty = match &mut self.command {
                    Some(command) => {
                        command.pop();
                        command.is_empty()
                    }
                    None => true,
                };
                if empty {
                    self.cancel_command(font);
                } else {
                    self.update_command(font);
                }
                true
            }
            Key::Escape => self.cancel_command(font),
            // LaTeX allows whitespace between a delimiter command and its
            // fence, so a space must not resolve the buffer and lose the fence.
            Key::Character(symbol)
                if symbol.is_whitespace()
                    && self.command.as_deref().is_some_and(|name| {
                        matches!(
                            command_shape(name),
                            CommandShape::Delimiter | CommandShape::SymbolArgument
                        )
                    }) =>
            {
                true
            }
            Key::Character(symbol)
                if self
                    .command
                    .as_deref()
                    .is_some_and(|name| command_shape(name) == CommandShape::Delimiter) =>
            {
                let name = self.command.clone().unwrap_or_default();
                match name.as_str() {
                    "left" => match left_delimiter_snippet(symbol) {
                        Some(snippet) => self.resolve_command_with(Some(snippet), font),
                        None => self.resolve_command(font),
                    },
                    "right" => self.close_delimiter(symbol, font),
                    "middle" => self.insert_middle(symbol, font),
                    _ => match delimiter_tex(symbol) {
                        Some(fence) => {
                            self.resolve_command_with(Some(format!("\\{name}{fence}")), font)
                        }
                        None => self.resolve_command(font),
                    },
                }
            }
            Key::Character(symbol)
                if self
                    .command
                    .as_deref()
                    .is_some_and(|name| command_shape(name) == CommandShape::SymbolArgument) =>
            {
                let name = self.command.clone().unwrap_or_default();
                self.resolve_command_with(Some(format!("\\{name}{symbol}")), font)
            }
            _ => {
                let resolved = self.resolve_command(font);
                if matches!(
                    key,
                    Key::Left | Key::Right | Key::Up | Key::Down | Key::Home | Key::End
                ) {
                    self.key(key, shift, font) || resolved
                } else {
                    true
                }
            }
        }
    }

    /// Keys inside the second `\` name, which only ends on whitespace or a
    /// key that is not a command letter.
    fn command_argument_key(&mut self, key: Key, shift: bool, font: &dyn MathFont) -> bool {
        match key {
            Key::Character(symbol) if is_command_argument_letter(symbol) => {
                if let Some(argument) = &mut self.command_argument {
                    argument.push(symbol);
                }
                self.command_ime_start = None;
                self.update_command(font);
                true
            }
            Key::Backspace => {
                let empty = match &mut self.command_argument {
                    Some(argument) => {
                        argument.pop();
                        argument.is_empty()
                    }
                    None => true,
                };
                if empty {
                    self.command_argument = None;
                }
                self.update_command(font);
                true
            }
            Key::Escape => self.cancel_command(font),
            Key::Character(symbol) if symbol.is_whitespace() => self.finish_command_argument(font),
            _ => {
                let resolved = self.finish_command_argument(font);
                if resolved {
                    self.key(key, shift, font) || true
                } else {
                    false
                }
            }
        }
    }

    /// Starts a command buffer and shows it as a `Composition` at the caret,
    /// so the text being typed takes layout space like any other content.
    fn begin_command(&mut self, font: &dyn MathFont) -> bool {
        if self.command.is_some() {
            return false;
        }
        self.normalize_spans();
        let before = self.snapshot();
        self.delete_selection_raw(font);
        let Some((row, index)) = self.insertion_row() else {
            self.restore(before);
            self.relayout(font);
            return false;
        };
        let node = match self.model.insert(
            row,
            Slot::Body,
            index,
            NodeKind::Composition("\\".to_owned()),
            Span::default(),
        ) {
            Ok(node) => node,
            Err(_) => {
                self.restore(before);
                self.relayout(font);
                return false;
            }
        };
        self.model.set_origin(node, Origin::Synthetic);
        self.command = Some(String::new());
        self.command_node = Some(node);
        self.command_before = Some(before);
        self.command_ime_start = None;
        self.caret = self
            .boundary_path(row, Slot::Body, index + 1)
            .unwrap_or_else(|| self.caret.clone());
        self.anchor = None;
        self.relayout(font);
        true
    }

    fn update_command(&mut self, font: &dyn MathFont) {
        let (Some(node), Some(name)) = (self.command_node, self.command.as_ref()) else {
            return;
        };
        let text = match &self.command_argument {
            Some(argument) => NodeKind::Composition(format!("\\{name}\\{argument}")),
            None => NodeKind::Composition(format!("\\{name}")),
        };
        if self.replace_kind(node, text).is_ok() {
            self.relayout(font);
        }
    }

    /// The IME is composing command letters (a Latin input mode, or pinyin
    /// before it commits). Replace its previous tail instead of cancelling the
    /// pending command, so `\` followed by an IME `f` stays `\f`.
    fn absorb_command_ime(&mut self, text: &str, font: &dyn MathFont) -> bool {
        let target = if let Some(argument) = self.command_argument.as_mut() {
            argument
        } else if let Some(command) = self.command.as_mut() {
            command
        } else {
            return false;
        };
        let base = self.command_ime_start.take().unwrap_or(target.len());
        target.truncate(base);
        target.push_str(text);
        if !text.is_empty() {
            self.command_ime_start = Some(base);
        }
        self.update_command(font);
        true
    }

    fn cancel_command(&mut self, font: &dyn MathFont) -> bool {
        let had_command = self.command.take().is_some();
        self.command_argument = None;
        self.command_ime_start = None;
        let node = self.command_node.take();
        let Some(before) = self.command_before.take() else {
            if let Some(node) = node
                && self.remove_node(node).is_ok()
            {
                self.relayout(font);
            }
            return had_command;
        };
        self.restore(before);
        self.relayout(font);
        true
    }

    fn resolve_command(&mut self, font: &dyn MathFont) -> bool {
        self.resolve_command_with(None, font)
    }

    /// Resolves the buffer, optionally replacing the parsed snippet. `\left`
    /// uses the override because its fence is the key that ended the buffer.
    fn resolve_command_with(&mut self, snippet: Option<String>, font: &dyn MathFont) -> bool {
        let Some(name) = self.command.take() else {
            return false;
        };
        self.command_argument = None;
        self.command_ime_start = None;
        let before = self.command_before.take();
        if let Some(node) = self.command_node.take() {
            let caret = self.caret_before_node(node);
            if self.remove_node(node).is_ok()
                && let Some(caret) = caret
            {
                self.caret = caret;
            }
        }
        if name.is_empty() {
            if let Some(before) = before {
                self.restore(before);
            }
            self.relayout(font);
            return true;
        }
        let snippet = snippet
            .or_else(|| command_template(&name))
            .unwrap_or_else(|| format!("\\{name}"));
        let document = parse(&snippet);
        if document.has_errors() {
            if let Some(before) = before {
                self.restore(before);
            }
            self.message = Some(format!("unknown command \\{name}"));
            self.relayout(font);
            return true;
        }
        let Some(before) = before else {
            return false;
        };
        self.push_undo(before);
        let Some((row, index)) = self.insertion_row() else {
            self.end();
            self.message = Some("cannot insert here".to_owned());
            return false;
        };
        let children: Vec<NodeId> = match document.model.kind(document.root()) {
            NodeKind::Row(children) => children.clone(),
            _ => vec![document.root()],
        };
        let mut inserted = Vec::new();
        for (offset, child) in children.into_iter().enumerate() {
            let copied = self.copy_parsed(&document, child);
            if self
                .model
                .insert_existing(row, Slot::Body, index + offset, copied)
                .is_err()
            {
                break;
            }
            inserted.push(copied);
        }
        if inserted.is_empty() {
            self.end();
            return false;
        }
        let empty = self.place_caret_after_insert(row, index, &inserted);
        if !empty {
            self.collapse_single_row(row);
        }
        self.relayout(font);
        self.end();
        true
    }

    /// Turns `\left\langle` and its relatives into one snippet or edit once
    /// the second name ends.
    fn finish_command_argument(&mut self, font: &dyn MathFont) -> bool {
        let Some(argument) = self.command_argument.take() else {
            return false;
        };
        let Some(name) = self.command.clone() else {
            return false;
        };
        if argument.is_empty() {
            self.cancel_command(font);
            self.message = Some(format!("`\\{name}` needs a command name"));
            return true;
        }
        if name == "not" {
            let snippet = format!("\\not\\{argument}");
            if parse(&snippet).has_errors() {
                self.cancel_command(font);
                self.message = Some(format!("`\\not` cannot precede \\{argument}"));
                return true;
            }
            return self.resolve_command_with(Some(snippet), font);
        }
        let Some(symbol) = delimiter_symbol(&argument) else {
            self.cancel_command(font);
            self.message = Some(format!("unknown delimiter \\{argument}"));
            return true;
        };
        match name.as_str() {
            "left" => self.resolve_command_with(Some(format!("\\left\\{argument}\\right.")), font),
            "right" => self.close_delimiter_with(Some(symbol), font),
            "middle" => self.insert_middle_with(symbol, font),
            _ => self.resolve_command_with(Some(format!("\\{name}\\{argument}")), font),
        }
    }

    /// Inserts a `\middle` fence; it is only valid inside a `\left` body.
    fn insert_middle(&mut self, symbol: char, font: &dyn MathFont) -> bool {
        let Some(Some(symbol)) = delimiter_fence(symbol) else {
            return self.resolve_command(font);
        };
        self.insert_middle_with(symbol, font)
    }

    fn insert_middle_with(&mut self, symbol: char, font: &dyn MathFont) -> bool {
        let enclosed = self.caret.steps.iter().any(|step| {
            step.slot == Slot::Body
                && matches!(self.model.kind(step.node), NodeKind::Delimited { .. })
        });
        if !enclosed {
            self.cancel_command(font);
            self.message = Some("`\\middle` needs an enclosing `\\left`".to_owned());
            return true;
        }
        if self.command.take().is_none() {
            return false;
        }
        self.command_argument = None;
        self.command_ime_start = None;
        let before = self.command_before.take();
        if let Some(node) = self.command_node.take()
            && let Some(caret) = self.caret_before_node(node)
            && self.remove_node(node).is_ok()
        {
            self.caret = caret;
        }
        let Some(before) = before else {
            return false;
        };
        self.push_undo(before);
        let Some((row, index)) = self.insertion_row() else {
            self.end();
            self.message = Some("cannot insert here".to_owned());
            return false;
        };
        let node = match self.model.insert(
            row,
            Slot::Body,
            index,
            NodeKind::Middle { symbol },
            Span::default(),
        ) {
            Ok(node) => node,
            Err(_) => {
                self.end();
                return false;
            }
        };
        self.model.set_origin(node, Origin::Synthetic);
        self.caret = self
            .boundary_path(row, Slot::Body, index + 1)
            .unwrap_or_else(|| self.caret.clone());
        self.anchor = None;
        self.relayout(font);
        self.end();
        true
    }

    /// Closes the nearest `\left` around the caret with the typed fence.
    fn close_delimiter(&mut self, symbol: char, font: &dyn MathFont) -> bool {
        let Some(right) = delimiter_fence(symbol) else {
            return self.resolve_command(font);
        };
        self.close_delimiter_with(right, font)
    }

    fn close_delimiter_with(&mut self, right: Option<char>, font: &dyn MathFont) -> bool {
        let delimited = self.caret.steps.iter().rev().find_map(|step| {
            (step.slot == Slot::Body
                && matches!(self.model.kind(step.node), NodeKind::Delimited { .. }))
            .then_some(step.node)
        });
        let Some(delimited) = delimited else {
            self.cancel_command(font);
            self.message = Some("`\\right` without `\\left`".to_owned());
            return true;
        };
        let Some(name) = self.command.take() else {
            return false;
        };
        debug_assert_eq!(name, "right");
        self.command_argument = None;
        self.command_ime_start = None;
        let before = self.command_before.take();
        if let Some(node) = self.command_node.take()
            && let Some(caret) = self.caret_before_node(node)
            && self.remove_node(node).is_ok()
        {
            self.caret = caret;
        }
        let NodeKind::Delimited { left, body, .. } = *self.model.kind(delimited) else {
            return false;
        };
        if self
            .replace_kind(delimited, NodeKind::Delimited { left, body, right })
            .is_err()
        {
            return false;
        }
        if let Some(before) = before {
            self.push_undo(before);
        }
        self.caret = self
            .caret_after_node(delimited)
            .unwrap_or_else(|| self.caret.clone());
        self.anchor = None;
        self.relayout(font);
        self.end();
        true
    }

    fn insert_atom(&mut self, symbol: char, font: &dyn MathFont) -> bool {
        self.begin();
        self.delete_selection_raw(font);
        let Some((row, index)) = self.insertion_row() else {
            self.end();
            return false;
        };
        let node = match self.model.insert(
            row,
            Slot::Body,
            index,
            NodeKind::Atom(Atom::new(symbol)),
            Span::default(),
        ) {
            Ok(node) => node,
            Err(_) => {
                self.end();
                return false;
            }
        };
        self.model.set_origin(node, Origin::Synthetic);
        self.caret = self
            .boundary_path(row, Slot::Body, index + 1)
            .unwrap_or_else(|| self.caret.clone());
        self.anchor = None;
        self.collapse_single_row(row);
        self.relayout(font);
        self.end();
        true
    }

    fn insert_script(&mut self, slot: Slot, font: &dyn MathFont) -> bool {
        self.begin();
        self.delete_selection_raw(font);
        let Some((row, index)) = self.insertion_row() else {
            self.end();
            return false;
        };
        if index > 0 {
            let previous = self.slot_children(row, Slot::Body).get(index - 1).copied();
            if let Some(previous) = previous
                && let NodeKind::Scripts { sup, sub, .. } = self.model.kind(previous)
            {
                let taken = if slot == Slot::Superscript {
                    sup.is_some()
                } else {
                    sub.is_some()
                };
                if taken {
                    self.end();
                    self.message = Some(format!("already has a {slot:?}"));
                    return false;
                }
                let empty = self.model.alloc_with_origin(
                    NodeKind::Row(Vec::new()),
                    Span::default(),
                    Origin::Synthetic,
                );
                if self.model.set_child(previous, slot, 0, empty).is_ok() {
                    self.caret = self
                        .boundary_path(previous, slot, 0)
                        .unwrap_or_else(|| self.caret.clone());
                    self.anchor = None;
                    self.relayout(font);
                    self.end();
                    return true;
                }
                self.end();
                return false;
            }
        }
        let empty = self.model.alloc_with_origin(
            NodeKind::Row(Vec::new()),
            Span::default(),
            Origin::Synthetic,
        );
        let (base, position) = if index > 0 {
            (
                self.slot_children(row, Slot::Body)[index - 1],
                Some(index - 1),
            )
        } else {
            (
                self.model.alloc_with_origin(
                    NodeKind::Row(Vec::new()),
                    Span::default(),
                    Origin::Synthetic,
                ),
                None,
            )
        };
        let (sup, sub) = if slot == Slot::Superscript {
            (Some(empty), None)
        } else {
            (None, Some(empty))
        };
        let scripts = self.model.alloc_with_origin(
            NodeKind::Scripts { base, sup, sub },
            Span::default(),
            Origin::Synthetic,
        );
        let placed = match position {
            Some(position) => self.model.set_child(row, Slot::Body, position, scripts),
            None => self.model.insert_existing(row, Slot::Body, 0, scripts),
        };
        if placed.is_err() {
            self.end();
            return false;
        }
        self.caret = self
            .boundary_path(scripts, slot, 0)
            .unwrap_or_else(|| self.caret.clone());
        self.anchor = None;
        self.relayout(font);
        self.end();
        true
    }

    fn insert_parsed_at(
        &mut self,
        row: NodeId,
        index: usize,
        source: &str,
        font: &dyn MathFont,
    ) -> bool {
        let document = parse(source);
        if document.has_errors() {
            self.message = Some("input contains an error".to_owned());
        }
        let children: Vec<NodeId> = match document.model.kind(document.root()) {
            NodeKind::Row(children) => children.clone(),
            _ => vec![document.root()],
        };
        let mut inserted = Vec::new();
        for (offset, child) in children.into_iter().enumerate() {
            let copied = self.copy_parsed(&document, child);
            if self
                .model
                .insert_existing(row, Slot::Body, index + offset, copied)
                .is_err()
            {
                break;
            }
            inserted.push(copied);
        }
        if inserted.is_empty() {
            return false;
        }
        let empty = self.place_caret_after_insert(row, index, &inserted);
        if !empty {
            self.collapse_single_row(row);
        }
        self.relayout(font);
        true
    }

    /// A single-child row in a one-child slot is an editor artifact, not a
    /// group the source had; collapse it so the export does not gain braces.
    fn collapse_single_row(&mut self, row: NodeId) -> bool {
        let children = self.slot_children(row, Slot::Body);
        if children.len() != 1 {
            return false;
        }
        let mut path = match self.path_to(row) {
            Some(path) => path,
            None => return false,
        };
        let Some(step) = path.pop() else {
            return false;
        };
        if self.model.kind(step.node).slot_is_list(step.slot) {
            return false;
        }
        if self
            .model
            .set_child(step.node, step.slot, step.index, children[0])
            .is_err()
        {
            return false;
        }
        self.caret = self
            .boundary_path(step.node, step.slot, step.index + 1)
            .unwrap_or_else(|| self.caret.clone());
        true
    }

    fn place_caret_after_insert(&mut self, row: NodeId, index: usize, inserted: &[NodeId]) -> bool {
        for (offset, node) in inserted.iter().enumerate() {
            if matches!(self.model.kind(*node), NodeKind::Row(children) if children.is_empty()) {
                self.caret = self
                    .boundary_path(row, Slot::Body, index + offset)
                    .unwrap_or_else(|| self.caret.clone());
                self.anchor = None;
                return true;
            }
            if let Some((node, slot, slot_index)) = first_empty_slot(&self.model, *node) {
                self.caret = self
                    .boundary_path(node, slot, slot_index)
                    .unwrap_or_else(|| self.caret.clone());
                self.anchor = None;
                return true;
            }
        }
        self.caret = self
            .boundary_path(row, Slot::Body, index + inserted.len())
            .unwrap_or_else(|| self.caret.clone());
        self.anchor = None;
        false
    }

    /// The row and index an insertion at the caret should use. A caret in a
    /// single-child slot enters the child, wrapping a leaf in a row first.
    fn insertion_row(&mut self) -> Option<(NodeId, usize)> {
        let step = *self.caret.steps.last()?;
        if matches!(self.model.kind(step.node), NodeKind::Row(_)) && step.slot == Slot::Body {
            return Some((step.node, step.index));
        }
        if matches!(self.model.kind(step.node), NodeKind::Table { .. }) && step.slot == Slot::Row {
            return None;
        }
        let children = self.slot_children(step.node, step.slot);
        let (child, after) = if step.index > 0 {
            (*children.get(step.index - 1)?, true)
        } else if let Some(child) = children.first() {
            (*child, false)
        } else {
            let row = self.model.alloc_with_origin(
                NodeKind::Row(Vec::new()),
                Span::default(),
                Origin::Synthetic,
            );
            self.model.set_child(step.node, step.slot, 0, row).ok()?;
            return Some((row, 0));
        };
        if matches!(self.model.kind(child), NodeKind::Row(_)) {
            let length = self.slot_children(child, Slot::Body).len();
            return Some((child, if after { length } else { 0 }));
        }
        let row = self.model.alloc_with_origin(
            NodeKind::Row(vec![child]),
            Span::default(),
            Origin::Synthetic,
        );
        let position = if after { step.index - 1 } else { step.index };
        self.model
            .set_child(step.node, step.slot, position, row)
            .ok()?;
        Some((row, if after { 1 } else { 0 }))
    }

    fn delete_selection_raw(&mut self, font: &dyn MathFont) -> bool {
        let Some(anchor) = self.anchor.clone() else {
            return false;
        };
        if anchor == self.caret {
            self.anchor = None;
            return false;
        }
        let (Some(anchor_index), Some(focus_index)) =
            (self.stop_index(&anchor), self.stop_index(&self.caret))
        else {
            self.anchor = None;
            return false;
        };
        let (low_index, low_path) = if anchor_index < focus_index {
            (anchor_index, anchor)
        } else {
            (focus_index, self.caret.clone())
        };
        self.anchor = None;
        let high_index = anchor_index.max(focus_index);
        let root = self.model.root();
        let deleted = self.delete_range_in(root, low_index, high_index);
        self.caret = low_path;
        if deleted {
            self.collapse_redundant_rows();
            self.relayout(font);
        }
        deleted
    }

    /// Removes every node whose stop interval lies inside `[low, high]`.
    ///
    /// Children are visited last to first so that removing a sibling does not
    /// change the slot indices of the nodes still to visit; the geometry index
    /// stays the pre-deletion one for the same reason. A node only partly in
    /// the range is descended into, because its required slots must survive.
    fn delete_range_in(&mut self, parent: NodeId, low: usize, high: usize) -> bool {
        let parent_kind = self.model.kind(parent).clone();
        let mut deleted = false;
        for (child, slot, index) in self.indexed_children(parent).into_iter().rev() {
            let Some((start, end)) = self.node_interval(parent, slot, index) else {
                continue;
            };
            if end <= low || start >= high {
                continue;
            }
            let inside = start >= low && end <= high;
            if inside && parent_kind.slot_is_list(slot) && self.remove_node(child).is_ok() {
                deleted = true;
                continue;
            }
            if self.model.kind(child).children().is_empty() {
                if inside && self.clear_single_slot(child) {
                    deleted = true;
                }
            } else if self.delete_range_in(child, low, high) {
                deleted = true;
            }
        }
        deleted
    }

    fn indexed_children(&self, parent: NodeId) -> Vec<(NodeId, Slot, usize)> {
        let mut indexed = Vec::new();
        let mut seen: Vec<(Slot, usize)> = Vec::new();
        for child in self.model.kind(parent).children() {
            let index = seen
                .iter()
                .find(|(slot, _)| *slot == child.slot)
                .map_or(0, |(_, count)| *count);
            match seen.iter_mut().find(|(slot, _)| *slot == child.slot) {
                Some((_, count)) => *count += 1,
                None => seen.push((child.slot, 1)),
            }
            indexed.push((child.id, child.slot, index));
        }
        indexed
    }

    /// The stop ordinals before and after the child at `(parent, slot, index)`,
    /// in the geometry that was current when the range deletion started.
    fn node_interval(&self, parent: NodeId, slot: Slot, index: usize) -> Option<(usize, usize)> {
        let start = self.boundary_path(parent, slot, index)?;
        let mut end = start.clone();
        end.steps.last_mut()?.index += 1;
        Some((self.stop_index(&start)?, self.stop_index(&end)?))
    }

    fn delete_before_raw(&mut self, font: &dyn MathFont) -> bool {
        let mut guard = self.geometry.carets().count() + 2;
        while guard > 0 {
            let Some(position) = self.stop_index(&self.caret) else {
                return false;
            };
            let Some(step) = self.caret.steps.last().copied() else {
                return false;
            };
            if step.index > 0 {
                let children = self.slot_children(step.node, step.slot);
                if let Some(candidate) = children.get(step.index - 1).copied() {
                    if self.model.kind(candidate).children().is_empty() {
                        if self.remove_leaf(candidate, step.node, step.slot) {
                            let mut caret = self.caret.clone();
                            if let Some(last) = caret.steps.last_mut() {
                                last.index -= 1;
                            }
                            self.caret = caret;
                        } else if is_empty_row(self.model.kind(candidate)) {
                            // An empty optional slot is removable as a whole;
                            // a required one has nothing left to clear.
                            if self.remove_node(candidate).is_err() {
                                return false;
                            }
                            self.caret = self
                                .caret_after_node(step.node)
                                .unwrap_or_else(|| self.caret.clone());
                        } else if !self.clear_single_slot(candidate) {
                            return false;
                        }
                        self.collapse_redundant_rows();
                        self.relayout(font);
                        return true;
                    }
                }
            }
            // Nothing removable directly before the caret: step to the
            // previous boundary and try there. This enters a preceding group
            // and crosses slot starts without deleting a whole structure.
            if position == 0 {
                return false;
            }
            let Some(previous) = self.stop_path(position - 1) else {
                return false;
            };
            self.caret = previous;
            guard -= 1;
        }
        false
    }

    fn delete_after_raw(&mut self, font: &dyn MathFont) -> bool {
        let mut guard = self.geometry.carets().count() + 2;
        while guard > 0 {
            let Some(position) = self.stop_index(&self.caret) else {
                return false;
            };
            let Some(step) = self.caret.steps.last().copied() else {
                return false;
            };
            let children = self.slot_children(step.node, step.slot);
            if let Some(candidate) = children.get(step.index).copied() {
                if self.model.kind(candidate).children().is_empty() {
                    if self.remove_leaf(candidate, step.node, step.slot) {
                        // The caret boundary is now before the next child.
                    } else if is_empty_row(self.model.kind(candidate)) {
                        if self.remove_node(candidate).is_err() {
                            return false;
                        }
                        self.caret = self
                            .caret_before_node(step.node)
                            .unwrap_or_else(|| self.caret.clone());
                    } else if !self.clear_single_slot(candidate) {
                        return false;
                    }
                    self.collapse_redundant_rows();
                    self.relayout(font);
                    return true;
                }
            }
            let Some(next) = self.stop_path(position + 1) else {
                return false;
            };
            self.caret = next;
            guard -= 1;
        }
        false
    }

    /// Drops a row that a deletion left holding one child in a single-child
    /// slot. The row was a caret container, so its braces are not content.
    fn collapse_redundant_rows(&mut self) {
        loop {
            let ids: Vec<NodeId> = self.model.ids().collect();
            let mut target = None;
            for parent in ids {
                for (child, slot, index) in self.indexed_children(parent) {
                    if self.model.kind(parent).slot_is_list(slot)
                        || matches!(
                            (self.model.kind(parent), slot),
                            (NodeKind::Delimited { .. }, Slot::Body)
                        )
                    {
                        // A delimiter body must stay a row: `\middle` is laid
                        // out only when it is a direct child of that row.
                        continue;
                    }
                    if let NodeKind::Row(children) = self.model.kind(child)
                        && let [only] = children.as_slice()
                    {
                        target = Some((parent, slot, index, child, *only));
                        break;
                    }
                }
                if target.is_some() {
                    break;
                }
            }
            let Some((parent, slot, index, row, only)) = target else {
                break;
            };
            if self.model.set_child(parent, slot, index, only).is_err() {
                break;
            }
            self.caret = remap_across_collapsed_row(&self.caret, row, parent, slot, index);
            if let Some(anchor) = &self.anchor {
                self.anchor = Some(remap_across_collapsed_row(anchor, row, parent, slot, index));
            }
            self.spellings.remove(row);
        }
    }

    /// Removes a leaf from a list slot. A leaf in a single-child slot cannot
    /// be removed; the caller clears it with an empty row instead.
    fn remove_leaf(&mut self, candidate: NodeId, parent: NodeId, slot: Slot) -> bool {
        if !self.model.kind(parent).slot_is_list(slot) {
            return false;
        }
        self.remove_node(candidate).is_ok()
    }

    fn clear_single_slot(&mut self, candidate: NodeId) -> bool {
        self.replace_kind(candidate, NodeKind::Row(Vec::new()))
            .is_ok()
    }

    fn caret_after_node(&self, node: NodeId) -> Option<CaretPath> {
        let mut steps = self.path_to(node)?;
        let last = steps.last_mut()?;
        last.index += 1;
        Some(CaretPath::new(steps))
    }

    fn caret_before_node(&self, node: NodeId) -> Option<CaretPath> {
        let steps = self.path_to(node)?;
        if steps.is_empty() {
            return None;
        }
        Some(CaretPath::new(steps))
    }

    fn move_horizontal(&mut self, delta: isize, shift: bool) -> bool {
        if !shift && self.anchor.is_some() {
            let (Some(anchor), Some(focus)) = (self.anchor_index(), self.caret_index()) else {
                self.anchor = None;
                return false;
            };
            let path = if delta < 0 {
                self.stop_path(anchor.min(focus))
            } else {
                self.stop_path(anchor.max(focus))
            };
            self.anchor = None;
            if let Some(path) = path {
                self.caret = path;
            }
            return true;
        }
        let Some(current) = self.caret_index() else {
            return false;
        };
        let count = self.geometry.carets().count();
        let target = if delta < 0 {
            current.saturating_sub(1)
        } else {
            (current + 1).min(count.saturating_sub(1))
        };
        let Some(path) = self.stop_path(target) else {
            return false;
        };
        self.move_to(path, shift);
        true
    }

    fn move_vertical(&mut self, direction: i32, shift: bool, font: &dyn MathFont) -> bool {
        let Some(rect) = self.caret_rect() else {
            return false;
        };
        if !shift {
            self.anchor = None;
        } else if self.anchor.is_none() {
            self.anchor = Some(self.caret.clone());
        }
        let em = self.style.em(font, self.size);
        let y = match direction {
            -1 => rect.top - 0.25 * em,
            _ => rect.bottom + 0.25 * em,
        };
        let limit = self.layout.ascent + self.layout.descent;
        let point = Point {
            x: rect.x.clamp(0.0, self.layout.width),
            y: y.clamp(0.0, limit),
        };
        let Some(path) = self.geometry.hit_test(point) else {
            return false;
        };
        self.caret = path;
        true
    }

    fn move_to_row_edge(&mut self, end: bool, shift: bool) -> bool {
        let Some(row) = self.current_row() else {
            return false;
        };
        let length = self.slot_children(row, Slot::Body).len();
        let index = if end { length } else { 0 };
        let Some(path) = self.boundary_path(row, Slot::Body, index) else {
            return false;
        };
        self.move_to(path, shift);
        true
    }

    fn move_to(&mut self, path: CaretPath, shift: bool) {
        if shift {
            if self.anchor.is_none() {
                self.anchor = Some(self.caret.clone());
            }
        } else {
            self.anchor = None;
        }
        self.caret = path;
    }

    fn current_row(&self) -> Option<NodeId> {
        let step = *self.caret.steps.last()?;
        if matches!(self.model.kind(step.node), NodeKind::Row(_)) && step.slot == Slot::Body {
            return Some(step.node);
        }
        let children = self.slot_children(step.node, step.slot);
        let child = if step.index > 0 {
            children.get(step.index - 1)
        } else {
            children.get(step.index)
        }?;
        matches!(self.model.kind(*child), NodeKind::Row(_)).then_some(*child)
    }

    fn slot_children(&self, node: NodeId, slot: Slot) -> Vec<NodeId> {
        if !self.model.is_alive(node) {
            return Vec::new();
        }
        self.model
            .kind(node)
            .children()
            .into_iter()
            .filter(|child| child.slot == slot)
            .map(|child| child.id)
            .collect()
    }

    fn boundary_path(&self, node: NodeId, slot: Slot, index: usize) -> Option<CaretPath> {
        let mut steps = self.path_to(node)?;
        steps.push(Step { node, slot, index });
        Some(CaretPath::new(steps))
    }

    fn path_to(&self, target: NodeId) -> Option<Vec<Step>> {
        if !self.model.is_alive(target) {
            return None;
        }
        let mut steps = Vec::new();
        if target == self.model.root() {
            return Some(steps);
        }
        if self.walk_to(self.model.root(), target, &mut steps) {
            Some(steps)
        } else {
            None
        }
    }

    fn walk_to(&self, node: NodeId, target: NodeId, steps: &mut Vec<Step>) -> bool {
        let mut counts: Vec<(Slot, usize)> = Vec::new();
        for child in self.model.kind(node).children() {
            let index = counts
                .iter()
                .find(|(slot, _)| *slot == child.slot)
                .map_or(0, |(_, count)| *count);
            steps.push(Step {
                node,
                slot: child.slot,
                index,
            });
            if child.id == target || self.walk_to(child.id, target, steps) {
                return true;
            }
            steps.pop();
            match counts.iter_mut().find(|(slot, _)| *slot == child.slot) {
                Some((_, count)) => *count += 1,
                None => counts.push((child.slot, 1)),
            }
        }
        false
    }

    fn stop_path(&self, index: usize) -> Option<CaretPath> {
        self.geometry
            .carets()
            .nth(index)
            .map(|(path, _)| path.clone())
    }

    fn stop_index(&self, path: &CaretPath) -> Option<usize> {
        self.geometry
            .carets()
            .position(|(candidate, _)| candidate == path)
    }

    fn caret_index(&self) -> Option<usize> {
        self.stop_index(&self.caret)
    }

    fn anchor_index(&self) -> Option<usize> {
        let anchor = self.anchor.as_ref()?;
        self.stop_index(anchor)
    }

    /// Copies a parsed subtree and the token spellings it carried.
    fn copy_parsed(&mut self, document: &Document, id: NodeId) -> NodeId {
        let kind = document
            .model
            .kind(id)
            .map_children(|child| self.copy_parsed(document, child));
        let copy = self
            .model
            .alloc_with_origin(kind, Span::default(), Origin::Synthetic);
        if let Some(spelling) = document.spellings.get(id) {
            self.spellings.insert(copy, spelling.clone());
        }
        copy
    }

    fn collect_subtree(&self, id: NodeId, out: &mut Vec<NodeId>) {
        out.push(id);
        for child in self.model.kind(id).children() {
            self.collect_subtree(child.id, out);
        }
    }

    /// Removes a subtree and drops the spellings of every node it freed, so a
    /// reused id cannot inherit another node's token.
    fn remove_node(&mut self, id: NodeId) -> Result<(), EditError> {
        let mut freed = Vec::new();
        self.collect_subtree(id, &mut freed);
        self.model.remove(id)?;
        for node in freed {
            self.spellings.remove(node);
        }
        Ok(())
    }

    fn replace_kind(&mut self, id: NodeId, kind: NodeKind) -> Result<(), EditError> {
        let mut old = Vec::new();
        self.collect_subtree(id, &mut old);
        let mut kept = Vec::new();
        for child in kind.children() {
            self.collect_subtree(child.id, &mut kept);
        }
        self.model.replace(id, kind)?;
        self.spellings.remove(id);
        for node in old {
            if !kept.contains(&node) {
                self.spellings.remove(node);
            }
        }
        Ok(())
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            model: self.model.clone(),
            spellings: self.spellings.clone(),
            caret: self.caret.clone(),
            anchor: self.anchor.clone(),
        }
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.model = snapshot.model;
        self.spellings = snapshot.spellings;
        self.caret = snapshot.caret;
        self.anchor = snapshot.anchor;
        self.parse_diagnostics.clear();
    }

    fn begin(&mut self) {
        // Snapshot the normalized spans too, or `end` would see the export's
        // offsets as an edit and keep a no-op undo step.
        self.normalize_spans();
        let snapshot = self.snapshot();
        self.push_undo(snapshot);
    }

    fn push_undo(&mut self, snapshot: Snapshot) {
        self.redo.clear();
        self.undo.push(snapshot);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.parse_diagnostics.clear();
    }

    fn end(&mut self) {
        if self
            .undo
            .last()
            .is_some_and(|snapshot| snapshot.model == self.model)
        {
            self.undo.pop();
        }
    }
}

fn is_command_letter(symbol: char) -> bool {
    symbol.is_ascii_alphabetic() || symbol == '@'
}

fn is_command_argument_letter(symbol: char) -> bool {
    is_command_letter(symbol) || symbol == '|'
}

fn is_command_text(text: &str) -> bool {
    text.chars().all(is_command_letter)
}

/// The delimiter key that ends `\left`, as it would appear in a snippet.
fn left_delimiter_snippet(symbol: char) -> Option<String> {
    let fence = delimiter_tex(symbol)?;
    Some(format!("\\left{fence}\\right."))
}

fn delimiter_tex(symbol: char) -> Option<&'static str> {
    Some(match symbol {
        '(' => "(",
        ')' => ")",
        '[' => "[",
        ']' => "]",
        '|' => "|",
        '/' => "/",
        '.' => ".",
        '{' => "\\{",
        '}' => "\\}",
        '<' => "\\langle",
        '>' => "\\rangle",
        _ => return None,
    })
}

fn delimiter_fence(symbol: char) -> Option<Option<char>> {
    Some(match symbol {
        '.' => None,
        '(' => Some('('),
        ')' => Some(')'),
        '[' => Some('['),
        ']' => Some(']'),
        '|' => Some('|'),
        '/' => Some('/'),
        '{' => Some('{'),
        '}' => Some('}'),
        '<' => Some('\u{27E8}'),
        '>' => Some('\u{27E9}'),
        _ => return None,
    })
}

/// Rewrites a caret path that crossed a row now replaced by its only child.
fn remap_across_collapsed_row(
    path: &CaretPath,
    row: NodeId,
    parent: NodeId,
    slot: Slot,
    row_index: usize,
) -> CaretPath {
    let Some(position) = path
        .steps
        .iter()
        .position(|step| step.node == row && step.slot == Slot::Body)
    else {
        return path.clone();
    };
    let index = path.steps[position].index;
    let mut steps = path.steps[..position].to_vec();
    steps.push(Step {
        node: parent,
        slot,
        index: row_index + usize::from(index > 0),
    });
    if index == 0 {
        steps.extend_from_slice(&path.steps[position + 1..]);
    }
    CaretPath::new(steps)
}

fn is_empty_row(kind: &NodeKind) -> bool {
    matches!(kind, NodeKind::Row(children) if children.is_empty())
}

/// The first empty row in a subtree, as the boundary that leads into it.
fn first_empty_slot(model: &Model, id: NodeId) -> Option<(NodeId, Slot, usize)> {
    let mut counts: Vec<(Slot, usize)> = Vec::new();
    for child in model.kind(id).children() {
        let index = counts
            .iter()
            .find(|(slot, _)| *slot == child.slot)
            .map_or(0, |(_, count)| *count);
        if let NodeKind::Row(children) = model.kind(child.id)
            && children.is_empty()
        {
            return Some((id, child.slot, index));
        }
        if let Some(found) = first_empty_slot(model, child.id) {
            return Some(found);
        }
        match counts.iter_mut().find(|(slot, _)| *slot == child.slot) {
            Some((_, count)) => *count += 1,
            None => counts.push((child.slot, 1)),
        }
    }
    None
}
