use std::collections::BTreeMap;

use super::{Collection, PackIcon};

#[derive(Debug, Clone, Default)]
pub struct Definition {
    pub module: String,
    pub glyphs: BTreeMap<Collection, Vec<PackIcon>>,
}
