use std::collections::BTreeMap;

use crate::model::{Collection, PackIcon};

pub(crate) fn glyphs_in_order(
    glyphs: &BTreeMap<Collection, Vec<PackIcon>>,
) -> Vec<(Collection, usize)> {
    let mut ordered = Vec::new();

    for (collection, packs) in glyphs {
        for (index, pack) in packs.iter().enumerate() {
            ordered.push((pack.order, collection.clone(), index));
        }
    }

    ordered.sort_by_key(|(order, _, _)| *order);
    ordered
        .into_iter()
        .map(|(_, collection, index)| (collection, index))
        .collect()
}
