mod generate_font;

use crate::generate_font::{font_path, wrap_iconify_svg};
use ::reqwest::Url;
use handlebars::Handlebars;
use reqwest::blocking as reqwest;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

const ICED_TEMPLATE: &str = include_str!("../templates/iced.rs.hbs");

const RESERVED_WORDS: [&str; 52] = [
    "as", "async", "await", "break", "const", "continue", "crate", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", "dyn", "abstract", "become", "box", "do", "final", "gen",
    "macro", "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
];

#[derive(Debug, Clone, Deserialize, PartialEq, PartialOrd, Eq, Ord)]
pub(crate) struct Collection(String);

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PackIcon {
    enum_variant: String,
    icon: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Definition {
    module: String,
    glyphs: BTreeMap<Collection, Vec<PackIcon>>,
}

pub enum GenType {
    Font,
    Iced,
}

pub struct Icon {
    path: PathBuf,
    gen_type: GenType,
    definition: Definition,
    hash: String,
}

impl Icon {
    pub fn builder(path: impl AsRef<Path>) -> Icon {
        let path = path.as_ref().to_path_buf();
        let content = fs::read_to_string(&path).unwrap_or_else(|err| {
            panic!("Failed to read file: {}", err);
        });

        #[derive(Debug, Clone, Serialize, Deserialize)]
        struct DefinitionTemp {
            module: String,
            glyphs: BTreeMap<String, String>,
        }

        let definition: DefinitionTemp = match toml::from_str(&content) {
            Ok(toml) => toml,
            Err(err) => {
                panic!("Failed to parse TOML: {}", err);
            }
        };

        let serialized = serde_json::to_vec(&definition).expect("Failed to serialize definition");
        let hash = hex_upper(&Sha256::digest(&serialized));

        let glyphs = definition
            .glyphs
            .into_iter()
            .map(|(enum_var, text)| {
                let (collection, icon) = text.split_once("::").unwrap_or_else(|| {
                    panic!(
                        "glyph '{}' must use 'collection::icon' syntax (got '{}')",
                        enum_var, text
                    )
                });
                (
                    collection.to_string(),
                    (reserved_name(enum_var), icon.to_string()),
                )
            })
            .fold(
                BTreeMap::<Collection, Vec<PackIcon>>::new(),
                |mut t, (lib, (enum_var, icon))| {
                    t.entry(Collection(lib)).or_default().push(PackIcon {
                        enum_variant: upper_first_char(&enum_var),
                        icon,
                    });
                    t
                },
            );

        let definition = Definition {
            module: definition.module,
            glyphs: glyphs,
        };

        Icon {
            path,
            gen_type: GenType::Font,
            definition,
            hash,
        }
    }

    pub fn set_gen_type(&mut self, gen_type: GenType) -> &mut Self {
        self.gen_type = gen_type;
        self
    }

    pub fn build(&mut self) {
        if self.up_to_date() {
            return;
        }

        fetch_icons(&mut self.definition.glyphs);
        generate_font::generate_font(
            &self.path,
            &self.definition.module,
            &mut self.definition.glyphs,
        );

        match self.gen_type {
            GenType::Font => return,
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
        match extract_hash(&module_path) {
            Some(existing) if existing == self.hash => true,
            _ => false,
        }
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

        let icons = self
            .definition
            .glyphs
            .values()
            .flat_map(|packs| packs.iter())
            .map(|pack| {
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

        if let Some(parent) = module_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).expect("failed to create module directories");
            }
        }

        fs::write(&module_path, rendered).expect("failed to write generated Iced module");
    }
}

fn fetch_icons(glyphs: &mut BTreeMap<Collection, Vec<PackIcon>>) {
    #[derive(Deserialize)]
    struct IconifyResponse {
        prefix: String,
        icons: BTreeMap<String, IconifyIcon>,
        width: f64,
        height: f64,
    }

    #[derive(Deserialize)]
    struct IconifyIcon {
        body: String,
    }

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

        let base = format!("https://api.iconify.design/{}.json", collection.0);
        let joined = wanted.join(",");

        let url = Url::parse_with_params(&base, &[("icons", joined)]).unwrap_or_else(|e| {
            panic!(
                "failed to build Iconify URL for collection '{}': {e}",
                collection.0
            )
        });

        let resp = reqwest::get(url)
            .unwrap_or_else(|e| {
                panic!(
                    "failed to GET Iconify for collection '{}': {e}",
                    collection.0
                )
            })
            .error_for_status()
            .unwrap_or_else(|e| {
                panic!(
                    "non-success HTTP status for collection '{}': {e}",
                    collection.0
                )
            });

        let parsed: IconifyResponse = resp.json().unwrap_or_else(|e| {
            panic!(
                "failed to parse Iconify JSON for collection '{}': {e}",
                collection.0
            )
        });

        if parsed.prefix != *collection.0 {
            panic!(
                "Iconify prefix mismatch: requested collection '{}', got '{}'",
                collection.0, parsed.prefix
            );
        }

        let fetched = (parsed.icons, parsed.width, parsed.height);

        for (pack, clean_name) in entries.iter_mut().zip(cleaned.into_iter()) {
            let (icons, width, height) = &fetched;
            let icon = icons.get(&clean_name).expect(&format!(
                "Iconify missing icon '{}' for collection '{}'",
                clean_name, collection.0,
            ));

            pack.icon = wrap_iconify_svg(&icon.body, *width, *height);
        }
    }
}

fn reserved_name(name: String) -> String {
    if RESERVED_WORDS.contains(&name.as_str()) {
        panic!("Reserved word used: {}", name);
    }
    name
}

fn upper_first_char(raw: &str) -> String {
    let mut chars = raw.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub(crate) fn module_leaf(module: &str) -> String {
    module_segments(module)
        .last()
        .cloned()
        .unwrap_or_else(|| module.to_string())
}

pub(crate) fn module_segments(module: &str) -> Vec<String> {
    let mut segments = Vec::new();

    for part in module.split("::") {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }

        segments.push(trimmed.to_string())
    }

    if segments.is_empty() && !module.is_empty() {
        segments.push(module.to_string());
    }

    segments
}

fn module_file_path(base: impl AsRef<Path>, module: &str) -> PathBuf {
    let mut path = PathBuf::from(base.as_ref());

    for segment in module_segments(module) {
        path.push(segment);
    }

    path.set_extension("rs");

    path
}

fn relative_path(from: &Path, to: &Path) -> PathBuf {
    use std::path::Component;

    let mut from_components = from.components().peekable();
    let mut to_components = to.components().peekable();

    while let (Some(f), Some(t)) = (from_components.peek(), to_components.peek()) {
        if *f == *t {
            from_components.next();
            to_components.next();
        } else {
            break;
        }
    }

    let mut relative = PathBuf::new();

    for component in from_components {
        match component {
            Component::Normal(_) | Component::ParentDir => relative.push(".."),
            _ => {}
        }
    }

    for component in to_components {
        relative.push(component.as_os_str());
    }

    if relative.as_os_str().is_empty() {
        relative.push(".");
    }

    relative
}

fn extract_hash(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("/// Icon hash (SHA-256):") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

fn hex_upper(bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut out, "{:02X}", byte).expect("write to string");
    }
    out
}
