use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct IconifyResponse {
    pub prefix: String,
    pub icons: BTreeMap<String, IconifyIcon>,
    pub width: f64,
    pub height: f64,
}

#[derive(Deserialize)]
pub(crate) struct IconifyIcon {
    pub body: String,
    pub width: Option<f64>,
    pub height: Option<f64>,
}
