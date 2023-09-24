use gloo_utils::document;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;
use web_sys::{Element, Event};

pub fn create_element(tag_name: &str) -> Result<Element, JsValue> {
  Ok(document().create_element(tag_name)?)
}

pub fn js_closure(rust_closure: impl FnMut(Event) + 'static) -> Closure<dyn FnMut(Event)> {
  Closure::wrap(Box::new(rust_closure) as Box<dyn FnMut(_)>)
}
