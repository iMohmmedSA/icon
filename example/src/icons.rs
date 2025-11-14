/// Generated automatically by build.rs
/// Do not edit manually.
/// Icon hash (SHA-256): 64DA0B71A709EB1D1F216723138E7110F888377EA8F1D0689E9BE2341552FB62
use iced::{Font, widget::Text};

pub const FONT: &[u8] = include_bytes!("../fonts/icons.ttf");

#[derive(Copy, Clone, Debug)]
pub enum Icons {
    Rust,
    Abc,
    Crab,
    Git,
}

impl Icons {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "\u{E000}",
            Self::Abc => "\u{E001}",
            Self::Crab => "\u{E002}",
            Self::Git => "\u{E003}",
        }
    }

    #[inline]
    pub fn symbol(self) -> Text<'static> {
        Text::new(self.as_str()).font(Font::with_name("icons"))
    }
}
