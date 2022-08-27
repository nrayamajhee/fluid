use wasm_bindgen::JsValue;
use web_sys::{Document, Element, Window};

pub fn window() -> Result<Window, JsValue> {
  Ok(web_sys::window().ok_or(JsValue::from("No window"))?)
}

pub fn document() -> Result<Document, JsValue> {
  Ok(window()?.document().ok_or(JsValue::from("No window"))?)
}

pub fn create_element(tag_name: &str) -> Result<Element, JsValue> {
  Ok(document()?.create_element(tag_name)?)
}
