use std::path::{Component, Path, PathBuf};

pub(crate) fn module_leaf(module: &str) -> String {
    module_segments(module)
        .last()
        .cloned()
        .unwrap_or_else(|| module.to_string())
}

fn module_segments(module: &str) -> Vec<String> {
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

pub(crate) fn module_file_path(base: impl AsRef<Path>, module: &str) -> PathBuf {
    let mut path = PathBuf::from(base.as_ref());

    for segment in module_segments(module) {
        path.push(segment);
    }

    path.set_extension("rs");

    path
}

pub(crate) fn relative_path(from: &Path, to: &Path) -> PathBuf {
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
