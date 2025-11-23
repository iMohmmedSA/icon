use crate::model::{Collection, Definition, PackIcon};
use crate::utils::{hex_upper, reserved_name, upper_first_char};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DefinitionTemp {
    module: String,
    glyphs: IndexMap<String, String>,
}

pub(crate) fn parse_definition(path: &Path) -> (Definition, String) {
    let content = fs::read_to_string(path).unwrap_or_else(|err| {
        panic!("Failed to read file: {}", err);
    });

    let definition: DefinitionTemp = match toml::from_str(&content) {
        Ok(toml) => toml,
        Err(err) => {
            panic!("Failed to parse TOML: {}", err);
        }
    };

    let serialized = serde_json::to_vec(&definition).expect("Failed to serialize definition");
    let hash = hex_upper(Sha256::digest(&serialized));

    let glyphs = definition
        .glyphs
        .into_iter()
        .enumerate()
        .map(|(order, (enum_var, text))| {
            let (collection, icon) = text.split_once("::").unwrap_or_else(|| {
                panic!(
                    "glyph '{}' must use 'collection::icon' syntax (got '{}')",
                    enum_var, text
                )
            });
            (
                order,
                Collection(collection.to_string()),
                reserved_name(enum_var),
                icon.to_string(),
            )
        })
        .fold(
            BTreeMap::<Collection, Vec<PackIcon>>::new(),
            |mut t, (order, collection, enum_var, icon)| {
                t.entry(collection).or_default().push(PackIcon {
                    enum_variant: upper_first_char(&enum_var),
                    icon,
                    order,
                });
                t
            },
        );

    let definition = Definition {
        module: definition.module,
        glyphs,
    };

    (definition, hash)
}
