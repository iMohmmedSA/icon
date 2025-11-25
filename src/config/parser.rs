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
    local_assets: IndexMap<String, String>,
}

pub(crate) fn parse_definition(path: &Path, assets_path: Option<&Path>) -> (Definition, String) {
    let content = fs::read_to_string(path).unwrap_or_else(|err| {
        panic!("Failed to read file: {}", err);
    });

    let definition: DefinitionTemp = toml::from_str(&content).unwrap_or_else(|err| {
        panic!("Failed to parse TOML: {}", err);
    });

    let serialized = serde_json::to_vec(&definition).expect("Failed to serialize definition");
    let hash = hex_upper(Sha256::digest(&serialized));

    let DefinitionTemp {
        module,
        glyphs: remote_glyphs,
        local_assets,
    } = definition;

    let mut glyphs = BTreeMap::<Collection, Vec<PackIcon>>::new();
    let remote_count = remote_glyphs.len();

    for (order, (enum_var, text)) in remote_glyphs.into_iter().enumerate() {
        let (collection, icon) = text.split_once("::").unwrap_or_else(|| {
            panic!(
                "glyph '{}' must use 'collection::icon' syntax (got '{}')",
                enum_var, text
            )
        });

        glyphs
            .entry(Collection {
                name: collection.to_string(),
                local: false,
            })
            .or_default()
            .push(PackIcon {
                enum_variant: upper_first_char(&reserved_name(enum_var)),
                icon: icon.to_string(),
                order,
            });
    }

    if let Some(assets_path) = assets_path {
        for (order, (enum_var, asset)) in local_assets.into_iter().enumerate() {
            let asset = asset.trim();
            if asset.is_empty() {
                panic!("Local asset for '{}' must not be empty", enum_var);
            }

            let asset_path = assets_path.join(asset).with_extension("svg");
            let svg = fs::read_to_string(&asset_path).unwrap_or_else(|err| {
                panic!(
                    "Failed to read local asset '{}': {}",
                    asset_path.display(),
                    err
                )
            });

            let svg = svg.trim();
            if svg.is_empty() {
                panic!("Local asset '{}' is empty", asset_path.display());
            }

            glyphs
                .entry(Collection {
                    name: "local".to_string(),
                    local: true,
                })
                .or_default()
                .push(PackIcon {
                    enum_variant: upper_first_char(&reserved_name(enum_var)),
                    icon: svg.to_string(),
                    order: remote_count + order,
                });
        }
    }

    let definition = Definition { module, glyphs };

    (definition, hash)
}
