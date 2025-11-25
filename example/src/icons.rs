/// Generated automatically by build.rs
/// Do not edit manually.
/// Icon hash (SHA-256): 15C30E8ABA11F0CC54A2E34DC2AE52B80E2BF84798178996F4E89A6BEB783384
use iced::{Font, widget::Text};

pub const FONT: &[u8] = include_bytes!("../fonts/icons.ttf");

#[derive(Copy, Clone, Debug)]
pub enum Icons {
    Rust,
    Abc,
    Crab,
    Git,
    StrokeTest,
}

impl Icons {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "\u{E000}",
            Self::Abc => "\u{E001}",
            Self::Crab => "\u{E002}",
            Self::Git => "\u{E003}",
            Self::StrokeTest => "\u{E004}",
        }
    }

    #[inline]
    pub fn symbol(self) -> Text<'static> {
        Text::new(self.as_str()).font(Font::with_name("icons"))
    }
}
