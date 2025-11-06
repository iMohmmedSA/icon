/// Generated automatically by build.rs
/// Do not edit manually.
/// Icon hash (SHA-256): 9D872D1F827CF22B61851736032640E868A3074C7A049EC110B12344FD13DE6E
use iced::{Font, widget::Text};

pub const FONT: &[u8] = include_bytes!("../fonts/icons.ttf");

#[derive(Copy, Clone, Debug)]
pub enum Icons {
    Rust,
    Crab,
    Abc,
}

impl Icons {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "\u{E000}",
            Self::Crab => "\u{E001}",
            Self::Abc => "\u{E002}",
        }
    }

    #[inline]
    pub fn symbol(self) -> Text<'static> {
        Text::new(self.as_str()).font(Font::with_name("icons"))
    }
}
