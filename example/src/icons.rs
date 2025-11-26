/// Generated automatically by build.rs
/// Do not edit manually.
/// Icon hash (SHA-256): 5327919B9C7F8D0BF14401B9586559E6D9DCC41FEFF9725E12ABBB1DCD12172F
use iced::{Font, widget::Text};

pub const FONT: &[u8] = include_bytes!("../fonts/icons.ttf");

#[derive(Copy, Clone, Debug)]
pub enum Icons {
    Rust,
    Crab,
    Git,
    StrokeTest,
    LayersTest,
}

impl Icons {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "\u{E000}",
            Self::Crab => "\u{E001}",
            Self::Git => "\u{E002}",
            Self::StrokeTest => "\u{E003}",
            Self::LayersTest => "\u{E004}",
        }
    }

    #[inline]
    pub fn symbol(self) -> Text<'static> {
        Text::new(self.as_str()).font(Font::with_name("icons"))
    }
}
