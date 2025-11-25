use crate::model::Collection;

use super::types::IconifyResponse;
use ::reqwest::Url;
use reqwest::blocking as reqwest;

pub(crate) fn fetch_collection(collection: &Collection, icons: &[&str]) -> IconifyResponse {
    let base = format!("https://api.iconify.design/{}.json", collection.name);
    let joined = icons.join(",");

    let url = Url::parse_with_params(&base, &[("icons", joined)]).unwrap_or_else(|e| {
        panic!(
            "failed to build Iconify URL for collection '{}': {e}",
            collection.name
        )
    });

    let resp = reqwest::get(url)
        .unwrap_or_else(|e| {
            panic!(
                "failed to GET Iconify for collection '{}': {e}",
                collection.name
            )
        })
        .error_for_status()
        .unwrap_or_else(|e| {
            panic!(
                "non-success HTTP status for collection '{}': {e}",
                collection.name
            )
        });

    resp.json().unwrap_or_else(|e| {
        panic!(
            "failed to parse Iconify JSON for collection '{}': {e}",
            collection.name
        )
    })
}
