use std::{fs, path::Path};

pub(crate) fn extract_hash(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("/// Icon hash (SHA-256):") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

pub(crate) fn hex_upper(bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut out, "{:02X}", byte).expect("write to string");
    }
    out
}
