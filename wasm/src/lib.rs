use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

#[wasm_bindgen]
pub struct Oxide86Computer {}

#[wasm_bindgen]
impl Oxide86Computer {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<Self, JsValue> {
        todo!("not implemented")
    }
}
