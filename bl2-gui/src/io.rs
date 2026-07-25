//! Platform save output. Native writes to disk (handled in `app.rs` via
//! `SaveFile::save`); the web build can't write to disk, so it downloads the
//! edited bytes as a file. This module holds the web download.

/// Trigger a browser download of `bytes` as a file named `name`.
#[cfg(target_arch = "wasm32")]
pub fn download(name: &str, bytes: &[u8]) {
    use eframe::wasm_bindgen::JsCast as _;

    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&array);

    let blob = match web_sys::Blob::new_with_u8_array_sequence(&parts) {
        Ok(b) => b,
        Err(_) => return,
    };
    let url = match web_sys::Url::create_object_url_with_blob(&blob) {
        Ok(u) => u,
        Err(_) => return,
    };

    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        if let Ok(el) = document.create_element("a") {
            if let Ok(anchor) = el.dyn_into::<web_sys::HtmlAnchorElement>() {
                anchor.set_href(&url);
                anchor.set_download(name);
                anchor.click();
            }
        }
    }
    let _ = web_sys::Url::revoke_object_url(&url);
}
