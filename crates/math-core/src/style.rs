//! TeX style levels and the transition table.

use crate::font::MathFont;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Size {
    Display,
    Text,
    Script,
    ScriptScript,
}

/// A style level; `cramped` is TeX's prime (`D'`, `T'`, …).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Style {
    pub size: Size,
    pub cramped: bool,
}

impl Style {
    pub const DISPLAY: Self = Style {
        size: Size::Display,
        cramped: false,
    };
    pub const DISPLAY_CRAMPED: Self = Style {
        size: Size::Display,
        cramped: true,
    };
    pub const TEXT: Self = Style {
        size: Size::Text,
        cramped: false,
    };
    pub const TEXT_CRAMPED: Self = Style {
        size: Size::Text,
        cramped: true,
    };
    pub const SCRIPT: Self = Style {
        size: Size::Script,
        cramped: false,
    };
    pub const SCRIPT_CRAMPED: Self = Style {
        size: Size::Script,
        cramped: true,
    };
    pub const SCRIPT_SCRIPT: Self = Style {
        size: Size::ScriptScript,
        cramped: false,
    };
    pub const SCRIPT_SCRIPT_CRAMPED: Self = Style {
        size: Size::ScriptScript,
        cramped: true,
    };

    /// Script and scriptscript styles use the tight spacing table.
    pub fn tight(self) -> bool {
        matches!(self.size, Size::Script | Size::ScriptScript)
    }

    pub fn display(self) -> bool {
        matches!(self.size, Size::Display)
    }

    pub fn cramped(self) -> Self {
        Style {
            cramped: true,
            ..self
        }
    }

    /// Superscript style (KaTeX `Style.sup`).
    pub fn sup(self) -> Self {
        match (self.size, self.cramped) {
            (Size::Display | Size::Text, false) => Self::SCRIPT,
            (Size::Display | Size::Text, true) => Self::SCRIPT_CRAMPED,
            (Size::Script | Size::ScriptScript, false) => Self::SCRIPT_SCRIPT,
            (Size::Script | Size::ScriptScript, true) => Self::SCRIPT_SCRIPT_CRAMPED,
        }
    }

    /// Subscript style (KaTeX `Style.sub`).
    pub fn sub(self) -> Self {
        match self.size {
            Size::Display | Size::Text => Self::SCRIPT_CRAMPED,
            Size::Script | Size::ScriptScript => Self::SCRIPT_SCRIPT_CRAMPED,
        }
    }

    /// Fraction numerator style (KaTeX `Style.fracNum`).
    pub fn frac_num(self) -> Self {
        match (self.size, self.cramped) {
            (Size::Display, false) => Self::TEXT,
            (Size::Display, true) => Self::TEXT_CRAMPED,
            (Size::Text, false) => Self::SCRIPT,
            (Size::Text, true) => Self::SCRIPT_CRAMPED,
            (Size::Script, false) => Self::SCRIPT_SCRIPT,
            (Size::Script, true) => Self::SCRIPT_SCRIPT_CRAMPED,
            (Size::ScriptScript, false) => Self::SCRIPT_SCRIPT,
            (Size::ScriptScript, true) => Self::SCRIPT_SCRIPT_CRAMPED,
        }
    }

    /// Fraction denominator style (KaTeX `Style.fracDen`).
    pub fn frac_den(self) -> Self {
        match self.size {
            Size::Display => Self::TEXT_CRAMPED,
            Size::Text => Self::SCRIPT_CRAMPED,
            Size::Script | Size::ScriptScript => Self::SCRIPT_SCRIPT_CRAMPED,
        }
    }

    /// The em size this style sets at, in the caller's units.
    ///
    /// The scale comes from MATH, never from a hard-coded 0.7: a face may
    /// declare its own script percentages.
    pub fn em(self, font: &dyn MathFont, size: f32) -> f32 {
        let constants = font.constants();
        let scale = match self.size {
            Size::Display | Size::Text => 1.0,
            Size::Script => constants.script_percent_scale_down,
            Size::ScriptScript => constants.script_script_percent_scale_down,
        };
        size * scale
    }
}

impl Default for Style {
    fn default() -> Self {
        Self::TEXT
    }
}
