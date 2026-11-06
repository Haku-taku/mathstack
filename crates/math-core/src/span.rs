//! Byte ranges into the document source.

/// A half-open byte range. Synthetic nodes use `0..0`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }

    /// A caret position with no extent.
    pub const fn empty(at: usize) -> Self {
        Span { start: at, end: at }
    }

    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }

    /// The smallest span covering both; an empty span never widens the result.
    pub fn join(self, other: Span) -> Span {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }
}
