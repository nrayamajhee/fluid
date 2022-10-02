use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;
use web_sys::{Document, Element, Event, HtmlElement, Window};

pub fn window() -> Result<Window, JsValue> {
  Ok(web_sys::window().ok_or(JsValue::from("No window object"))?)
}

pub fn document() -> Result<Document, JsValue> {
  Ok(
    window()?
      .document()
      .ok_or(JsValue::from("Window has no document"))?,
  )
}

pub fn body() -> Result<HtmlElement, JsValue> {
  Ok(
    document()?
      .body()
      .ok_or(JsValue::from("Document has no body"))?,
  )
}

pub fn create_element(tag_name: &str) -> Result<Element, JsValue> {
  Ok(document()?.create_element(tag_name)?)
}

pub fn js_closure(rust_closure: impl FnMut(Event) + 'static) -> Closure<dyn FnMut(Event)> {
  Closure::wrap(Box::new(rust_closure) as Box<dyn FnMut(_)>)
}
