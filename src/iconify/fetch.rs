use std::collections::{BTreeMap, HashSet};

use crate::{
    generator::font::wrap_iconify_svg,
    model::{Collection, PackIcon},
};

use super::client::fetch_collection;

pub(crate) fn fetch_icons(glyphs: &mut BTreeMap<Collection, Vec<PackIcon>>) {
    for (collection, entries) in glyphs.iter_mut() {
        let cleaned: Vec<String> = entries
            .iter()
            .map(|pack| {
                let trimmed = pack.icon.trim();
                if trimmed.is_empty() {
                    panic!(
                        "Icon '{}' for collection '{:?}' must not be empty",
                        pack.enum_variant, collection
                    );
                }
                trimmed.to_string()
            })
            .collect();

        let mut seen = HashSet::new();
        let wanted: Vec<&str> = cleaned
            .iter()
            .map(|s| s.as_str())
            .filter(|name| seen.insert(*name))
            .collect();

        let parsed = fetch_collection(collection, &wanted);

        if parsed.prefix != *collection.0 {
            panic!(
                "Iconify prefix mismatch: requested collection '{}', got '{}'",
                collection.0, parsed.prefix
            );
        }

        let fetched = (parsed.icons, parsed.width, parsed.height);

        for (pack, clean_name) in entries.iter_mut().zip(cleaned.into_iter()) {
            let (ref icons, width, height) = fetched;
            let icon = icons.get(&clean_name).unwrap_or_else(|| {
                panic!(
                    "Iconify missing icon '{}' for collection '{}'",
                    clean_name, collection.0
                )
            });

            pack.icon = wrap_iconify_svg(
                &icon.body,
                icon.width.unwrap_or(width),
                icon.height.unwrap_or(height),
            );
        }
    }
}
