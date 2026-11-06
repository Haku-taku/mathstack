//! Caret paths, caret rectangles, hit testing and selection over a laid-out
//! formula.
//!
//! The geometry index is built from the box tree before it is flattened for
//! painting, so every rectangle refers to the model node that produced it.
//! Coordinates are the caller's units with y down from the formula's top edge.

use crate::layout::{MathBox, Placed};
use crate::node::{Model, NodeId, NodeKind, Slot};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn right(self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    pub fn center_x(self) -> f32 {
        self.x + self.width / 2.0
    }

    fn contains(self, point: Point) -> bool {
        point.x >= self.x
            && point.x <= self.right()
            && point.y >= self.y
            && point.y <= self.bottom()
    }

    fn union(self, other: Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Rect {
            x,
            y,
            width: self.right().max(other.right()) - x,
            height: self.bottom().max(other.bottom()) - y,
        }
    }
}

/// A vertical caret, spanning one line of the box tree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaretRect {
    pub x: f32,
    pub top: f32,
    pub bottom: f32,
}

/// A required slot that holds an empty row. In an editing layout the slot
/// reserves `rect`; a host paints it as the empty-slot placeholder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmptySlot {
    pub node: NodeId,
    pub rect: Rect,
    pub em: f32,
}

/// One step of a caret path: a boundary inside a node's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Step {
    pub node: NodeId,
    pub slot: Slot,
    pub index: usize,
}

/// A serializable caret address. The first step names the root; each step
/// selects a child of the previous step's node, and the last step is the
/// boundary between children (or after the final child).
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CaretPath {
    pub steps: Vec<Step>,
}

impl CaretPath {
    pub fn new(steps: Vec<Step>) -> Self {
        CaretPath { steps }
    }
}

#[derive(Clone)]
struct GNode {
    id: Option<NodeId>,
    rect: Rect,
    baseline: f32,
    slots: Vec<GSlot>,
}

#[derive(Clone)]
struct GSlot {
    slot: Slot,
    children: Vec<GChild>,
}

#[derive(Clone)]
struct GChild {
    id: Option<NodeId>,
    rect: Rect,
    node: Option<GNode>,
}

#[derive(Clone)]
enum Entry {
    Stop(usize),
    Ink(Rect),
}

/// The editing geometry of one laid-out formula.
pub struct Geometry {
    root: GNode,
    /// Node id to its box, indexed by `NodeId::index`.
    boxes: Vec<Option<Rect>>,
    stops: Vec<(CaretPath, CaretRect)>,
    entries: Vec<Entry>,
    empty_slots: Vec<EmptySlot>,
    em: f32,
}

impl Geometry {
    pub(crate) fn new(model: &Model, body: &MathBox, em: f32) -> Self {
        let mut boxes = vec![None; model.slots()];
        let mut empty_slots = Vec::new();
        let root = convert(model, body, 0.0, body.ascent, &mut boxes, &mut empty_slots);
        let mut collector = Collector {
            em,
            stops: Vec::new(),
            entries: Vec::new(),
        };
        let mut prefix = Vec::new();
        let bounds = root.rect;
        collector.node(&root, &mut prefix, bounds);
        if let (Some(node), Some(em)) = (body.id, body.empty_em) {
            empty_slots.push(EmptySlot {
                node,
                rect: root.rect,
                em,
            });
        }
        Geometry {
            root,
            boxes,
            stops: collector.stops,
            entries: collector.entries,
            empty_slots,
            em,
        }
    }

    /// The box of `id`, in formula coordinates.
    pub fn rect_of(&self, id: NodeId) -> Option<Rect> {
        self.boxes.get(id.index()).copied().flatten()
    }

    /// Every required slot holding an empty row, in reading order.
    pub fn empty_slots(&self) -> impl Iterator<Item = EmptySlot> + '_ {
        self.empty_slots.iter().copied()
    }

    /// Every caret the layout offers, in reading order.
    pub fn carets(&self) -> impl Iterator<Item = &(CaretPath, CaretRect)> {
        self.stops.iter()
    }

    /// Resolves a caret path, or `None` when a step does not name the node it
    /// claims or an index is out of range.
    pub fn caret(&self, path: &CaretPath) -> Option<CaretRect> {
        let (first, _) = path.steps.split_first()?;
        if Some(first.node) != self.root.id {
            return None;
        }
        let mut node = &self.root;
        let mut bounds = node.rect;
        for (position, step) in path.steps.iter().enumerate() {
            if Some(step.node) != node.id {
                return None;
            }
            let children: Vec<&GChild> = node
                .slots
                .iter()
                .find(|slot| slot.slot == step.slot)?
                .children
                .iter()
                .filter(|child| child.id.is_some())
                .collect();
            if step.index > children.len() {
                return None;
            }
            if position + 1 == path.steps.len() {
                return Some(boundary(node, step.slot, step.index, self.em, bounds));
            }
            if node.rect.height > 0.01 {
                bounds = node.rect;
            }
            let child = children.get(step.index)?;
            node = child.node.as_ref()?;
        }
        None
    }

    /// The caret closest to `point`, biased to the midpoint of the innermost
    /// box that contains it.
    pub fn hit_test(&self, point: Point) -> Option<CaretPath> {
        let mut steps = Vec::new();
        self.hit(&self.root, point, &mut steps);
        if steps.is_empty() {
            return None;
        }
        Some(CaretPath { steps })
    }

    /// Rectangles covering the content between two carets, merged per line.
    pub fn selection(&self, anchor: &CaretPath, focus: &CaretPath) -> Vec<Rect> {
        let Some(anchor) = self.stop_index(anchor) else {
            return Vec::new();
        };
        let Some(focus) = self.stop_index(focus) else {
            return Vec::new();
        };
        let (low, high) = if anchor <= focus {
            (anchor, focus)
        } else {
            (focus, anchor)
        };
        let mut kept = Vec::new();
        let mut previous = None;
        for entry in &self.entries {
            match entry {
                Entry::Stop(index) => previous = Some(*index),
                Entry::Ink(rect) => {
                    if let Some(index) = previous
                        && low <= index
                        && index < high
                    {
                        kept.push(*rect);
                    }
                }
            }
        }
        merge_lines(kept)
    }

    fn stop_index(&self, path: &CaretPath) -> Option<usize> {
        self.stops
            .iter()
            .position(|(candidate, _)| candidate == path)
    }

    fn hit(&self, node: &GNode, point: Point, steps: &mut Vec<Step>) -> bool {
        let Some(id) = node.id else {
            return false;
        };
        let Some(slot) = choose_slot(node, point) else {
            return false;
        };
        let children: Vec<&GChild> = node
            .slots
            .iter()
            .find(|candidate| candidate.slot == slot)
            .map(|slot| {
                slot.children
                    .iter()
                    .filter(|child| child.id.is_some())
                    .collect()
            })
            .unwrap_or_default();
        // A table's rows stack vertically, so their midpoint is on y; every
        // other slot lays its children out along x.
        let by_y = slot == Slot::Row;
        let along = if by_y { point.y } else { point.x };
        let mut index = children.len();
        for (position, child) in children.iter().enumerate() {
            let midpoint = if by_y {
                child.rect.y + child.rect.height / 2.0
            } else {
                child.rect.center_x()
            };
            if along < midpoint {
                index = position;
                break;
            }
        }
        steps.push(Step {
            node: id,
            slot,
            index,
        });
        // The insertion index is a boundary; the containing child can be on
        // either side of it. Descending records the child's own index, which
        // is what makes the path resolve back to the same caret.
        let inside = children
            .get(index)
            .filter(|child| child.rect.contains(point))
            .map(|child| (index, child))
            .or_else(|| {
                index.checked_sub(1).and_then(|position| {
                    children
                        .get(position)
                        .filter(|child| child.rect.contains(point))
                        .map(|child| (position, child))
                })
            });
        if let Some((position, child)) = inside
            && let Some(child_node) = &child.node
            && !child_node.slots.is_empty()
        {
            if let Some(last) = steps.last_mut() {
                last.index = position;
            }
            self.hit(child_node, point, steps);
        }
        true
    }
}

fn boundary(node: &GNode, slot: Slot, index: usize, em: f32, bounds: Rect) -> CaretRect {
    let children: Vec<&GChild> = node
        .slots
        .iter()
        .find(|candidate| candidate.slot == slot)
        .map(|slot| {
            slot.children
                .iter()
                .filter(|child| child.id.is_some())
                .collect()
        })
        .unwrap_or_default();
    let x = match children.get(index) {
        Some(child) => child.rect.x,
        None => children
            .last()
            .map_or(node.rect.x, |child| child.rect.right()),
    };
    let mut top = f32::INFINITY;
    let mut bottom = f32::NEG_INFINITY;
    for child in &children {
        top = top.min(child.rect.y);
        bottom = bottom.max(child.rect.bottom());
    }
    if !top.is_finite() || bottom - top < 0.8 * em {
        let center = if top.is_finite() {
            (top + bottom) / 2.0
        } else {
            node.baseline
        };
        // An empty slot has no ink to size the caret; a minimum height keeps
        // it visible, clamped to the nearest non-empty box so it stays inside
        // the formula. An empty row is its own zero-height container, so the
        // ancestor bound is the one that applies.
        top = center - 0.5 * em;
        bottom = center + 0.5 * em;
        let clamp = if node.rect.height > 0.01 {
            node.rect
        } else {
            bounds
        };
        if clamp.height > 0.01 {
            top = top.max(clamp.y);
            bottom = bottom.min(clamp.bottom());
        }
    }
    CaretRect { x, top, bottom }
}

struct Collector {
    em: f32,
    stops: Vec<(CaretPath, CaretRect)>,
    entries: Vec<Entry>,
}

impl Collector {
    fn node(&mut self, node: &GNode, prefix: &mut Vec<Step>, bounds: Rect) {
        let Some(id) = node.id else {
            return;
        };
        let bounds = if node.rect.height > 0.01 {
            node.rect
        } else {
            bounds
        };
        for slot in &node.slots {
            let ids: Vec<&GChild> = slot
                .children
                .iter()
                .filter(|child| child.id.is_some())
                .collect();
            for child in slot.children.iter().filter(|child| child.id.is_none()) {
                self.entries.push(Entry::Ink(child.rect));
            }
            self.stop(id, slot.slot, 0, node, prefix, bounds);
            for (index, child) in ids.iter().enumerate() {
                if let Some(child_node) = &child.node
                    && !child_node.slots.is_empty()
                {
                    prefix.push(Step {
                        node: id,
                        slot: slot.slot,
                        index,
                    });
                    self.node(child_node, prefix, bounds);
                    prefix.pop();
                } else {
                    self.entries.push(Entry::Ink(child.rect));
                }
                self.stop(id, slot.slot, index + 1, node, prefix, bounds);
            }
        }
    }

    fn stop(
        &mut self,
        id: NodeId,
        slot: Slot,
        index: usize,
        node: &GNode,
        prefix: &[Step],
        bounds: Rect,
    ) {
        let mut steps = prefix.to_vec();
        steps.push(Step {
            node: id,
            slot,
            index,
        });
        let rect = boundary(node, slot, index, self.em, bounds);
        self.stops.push((CaretPath { steps }, rect));
        let stop = self.stops.len() - 1;
        self.entries.push(Entry::Stop(stop));
    }
}

/// The slot a point falls into, by the node's own geometry; a fractional
/// fallback picks the nearest slot centre.
fn choose_slot(node: &GNode, point: Point) -> Option<Slot> {
    let has = |slot: Slot| node.slots.iter().any(|candidate| candidate.slot == slot);
    if !has(Slot::Superscript) && !has(Slot::Subscript) && !has(Slot::Numerator) && !has(Slot::Over)
    {
        return node.slots.first().map(|slot| slot.slot);
    }
    let base = slot_rect(node, Slot::Base);
    if has(Slot::Superscript) || has(Slot::Subscript) {
        if let Some(base) = base {
            if point.y < base.y && has(Slot::Superscript) {
                return Some(Slot::Superscript);
            }
            if point.y > base.bottom() && has(Slot::Subscript) {
                return Some(Slot::Subscript);
            }
            return Some(Slot::Base);
        }
    }
    if has(Slot::Numerator) {
        return Some(if point.y < node.baseline {
            Slot::Numerator
        } else {
            Slot::Denominator
        });
    }
    if has(Slot::Over) || has(Slot::Under) {
        if let Some(base) = base {
            if point.y < base.y && has(Slot::Over) {
                return Some(Slot::Over);
            }
            if point.y > base.bottom() && has(Slot::Under) {
                return Some(Slot::Under);
            }
        }
    }
    nearest_slot(node, point)
}

fn slot_rect(node: &GNode, slot: Slot) -> Option<Rect> {
    let slot = node.slots.iter().find(|candidate| candidate.slot == slot)?;
    let mut rect: Option<Rect> = None;
    for child in &slot.children {
        rect = Some(match rect {
            Some(rect) => rect.union(child.rect),
            None => child.rect,
        });
    }
    rect
}

fn nearest_slot(node: &GNode, point: Point) -> Option<Slot> {
    let mut best: Option<(f32, Slot)> = None;
    for slot in &node.slots {
        let Some(rect) = slot_rect(node, slot.slot) else {
            continue;
        };
        let center = rect.y + rect.height / 2.0;
        let distance = (center - point.y).abs();
        if best.is_none_or(|(best, _)| distance < best) {
            best = Some((distance, slot.slot));
        }
    }
    best.map(|(_, slot)| slot)
}

fn convert(
    model: &Model,
    box_: &MathBox,
    x: f32,
    baseline: f32,
    boxes: &mut Vec<Option<Rect>>,
    empty_slots: &mut Vec<EmptySlot>,
) -> GNode {
    let rect = Rect {
        x,
        y: baseline - box_.ascent,
        width: box_.width,
        height: box_.ascent + box_.descent,
    };
    if let Some(id) = box_.id {
        if let Some(slot) = boxes.get_mut(id.index()) {
            *slot = Some(rect);
        }
    }
    let mut slots: Vec<GSlot> = Vec::new();
    for placed in &box_.children {
        let child = convert_placed(model, placed, x, baseline, box_.id, boxes, empty_slots);
        match slots.iter_mut().find(|slot| slot.slot == placed.slot) {
            Some(slot) => slot.children.push(child),
            None => slots.push(GSlot {
                slot: placed.slot,
                children: vec![child],
            }),
        }
    }
    // An empty row still needs a boundary: without one, an empty formula has
    // no caret at all and a host cannot start typing into it.
    if slots.is_empty()
        && box_
            .id
            .is_some_and(|id| matches!(model.kind(id), NodeKind::Row(_)))
    {
        slots.push(GSlot {
            slot: Slot::Body,
            children: Vec::new(),
        });
    }
    GNode {
        id: box_.id,
        rect,
        baseline,
        slots,
    }
}

fn convert_placed(
    model: &Model,
    placed: &Placed,
    x: f32,
    baseline: f32,
    parent: Option<NodeId>,
    boxes: &mut Vec<Option<Rect>>,
    empty_slots: &mut Vec<EmptySlot>,
) -> GChild {
    let child_x = x + placed.dx;
    let child_baseline = baseline + placed.dy;
    let node = placed.child.id.map(|_| {
        convert(
            model,
            &placed.child,
            child_x,
            child_baseline,
            boxes,
            empty_slots,
        )
    });
    let rect = Rect {
        x: child_x,
        y: child_baseline - placed.child.ascent,
        width: placed.child.width,
        height: placed.child.ascent + placed.child.descent,
    };
    if let (Some(node), Some(em)) = (placed.child.id, placed.child.empty_em) {
        let listed = parent.is_some_and(|parent| model.kind(parent).slot_is_list(placed.slot));
        if !listed {
            empty_slots.push(EmptySlot { node, rect, em });
        }
    }
    GChild {
        id: placed.child.id,
        rect,
        node,
    }
}

/// Unions rectangles that share a line, in top-to-bottom order.
fn merge_lines(mut rects: Vec<Rect>) -> Vec<Rect> {
    rects.sort_by(|a, b| a.y.total_cmp(&b.y));
    let mut merged: Vec<Rect> = Vec::new();
    for rect in rects {
        match merged.last_mut() {
            Some(last) if rect.y <= last.bottom() && rect.bottom() >= last.y => {
                *last = last.union(rect);
            }
            _ => merged.push(rect),
        }
    }
    merged
}
