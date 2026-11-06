//! A color a display item can carry.

/// Straight (not premultiplied) sRGB with an alpha channel; 255 is opaque.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Color {
    pub const BLACK: Color = Color::rgb(0, 0, 0);

    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Color {
            red,
            green,
            blue,
            alpha: 255,
        }
    }

    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Color {
            red,
            green,
            blue,
            alpha,
        }
    }

    pub const fn is_opaque(self) -> bool {
        self.alpha == 255
    }
}
