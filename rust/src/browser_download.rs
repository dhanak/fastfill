//! Tiny browser adapter; normal download UI belongs to the browser.

use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

#[wasm_bindgen(module = "/web/download.mjs")]
extern "C" {
    #[wasm_bindgen(catch, js_name = downloadFile)]
    fn start_download(
        name: &str,
        bytes: &[u8],
        mime_type: &str,
    ) -> Result<(), JsValue>;
}

pub(crate) fn download(
    name: &str,
    bytes: &[u8],
    mime_type: &str,
) -> Result<(), String> {
    start_download(name, bytes, mime_type).map_err(|_| {
        format!(
            "Could not start download of {name}. \
             Check your browser's download permissions."
        )
    })
}
