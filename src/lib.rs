mod config;
mod generator;
mod iconify;
mod model;
mod utils;

pub use model::GenType;

use crate::config::parse_definition;
use crate::generator::{font_path, generate_font};
use crate::iconify::fetch_icons;
use crate::model::Definition;
use crate::utils::{
    extract_hash, glyphs_in_order, module_file_path, relative_path, upper_first_char,
};
use handlebars::Handlebars;
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
};

const ICED_TEMPLATE: &str = include_str!("../templates/iced.rs.hbs");

pub struct Icon {
    path: PathBuf,
    assets_path: Option<PathBuf>,

    gen_type: GenType,
    definition: Definition,
    hash: String,
}

impl Icon {
    pub fn builder(path: impl AsRef<Path>) -> Icon {
        let path = path.as_ref().to_path_buf();
        Icon {
            path,
            assets_path: None,
            gen_type: GenType::Font,
            definition: Default::default(),
            hash: Default::default(),
        }
    }

    pub fn set_assets_path(&mut self, assets_path: impl AsRef<Path>) -> &mut Self {
        self.assets_path = Some(assets_path.as_ref().to_path_buf());
        self
    }

    pub fn set_gen_type(&mut self, gen_type: GenType) -> &mut Self {
        self.gen_type = gen_type;
        self
    }

    pub fn build(&mut self) {
        let (definition, hash) = parse_definition(&self.path, self.assets_path.as_deref());
        self.definition = definition;
        self.hash = hash;

        if self.up_to_date() {
            return;
        }

        fetch_icons(&mut self.definition.glyphs);
        generate_font(
            &self.path,
            &self.definition.module,
            &mut self.definition.glyphs,
        );

        match self.gen_type {
            GenType::Font => (),
            GenType::Iced => self.generate_iced(),
        }
    }

    fn up_to_date(&mut self) -> bool {
        let (font_path, _) = font_path(&self.path, &self.definition.module);
        if !font_path.exists() {
            return false;
        }

        if matches!(self.gen_type, GenType::Font) {
            return true;
        }

        let module_path = module_file_path("src", &self.definition.module);
        matches!(extract_hash(&module_path), Some(existing) if existing == self.hash)
    }

    fn generate_iced(&mut self) {
        let module_path = module_file_path("src", &self.definition.module);
        let (font_file_path, module_basename) = font_path(&self.path, &self.definition.module);
        if !font_file_path.exists() {
            panic!(
                "font file '{}' missing; run build with GenType::Font at least once",
                font_file_path.display()
            );
        }

        let module_dir = module_path.parent().unwrap_or_else(|| Path::new(""));
        let module = upper_first_char(&module_basename);

        let font_include = relative_path(module_dir, &font_file_path)
            .display()
            .to_string()
            .replace('\\', "/");
        let font_name = font_file_path
            .file_stem()
            .expect("font file path missing file stem")
            .to_string_lossy()
            .to_string();

        let glyphs = &self.definition.glyphs;
        let icons = glyphs_in_order(glyphs)
            .into_iter()
            .map(|(collection, index)| {
                let pack = glyphs
                    .get(&collection)
                    .and_then(|packs| packs.get(index))
                    .unwrap_or_else(|| {
                        panic!("glyph order mismatch for collection '{}'", collection.name)
                    });

                let ch = pack
                    .icon
                    .chars()
                    .next()
                    .expect("icon missing generated codepoint");
                json!({
                    "variant": pack.enum_variant,
                    "codepoint": format!("\\u{{{:04X}}}", ch as u32),
                })
            })
            .collect::<Vec<_>>();

        let data = json!({
            "module": module,
            "font_include": font_include,
            "font_name": font_name,
            "icon_hash": &self.hash,
            "icons": icons,
        });

        let handlebars = Handlebars::new();
        let rendered = handlebars
            .render_template(ICED_TEMPLATE, &data)
            .expect("failed to render Iced template");

        if let Some(parent) = module_path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).expect("failed to create module directories");
        }

        fs::write(&module_path, rendered).expect("failed to write generated Iced module");
    }
}
