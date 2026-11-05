//! Let queued progress messages and browser painting complete before CPU work.

#[wasm_bindgen::prelude::wasm_bindgen(module = "/web/progress.mjs")]
extern "C" {
    #[wasm_bindgen(js_name = yieldForPaint)]
    pub(crate) async fn yield_for_paint();
}
