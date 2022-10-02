use fluid::{body, document, Context};
use fluid_macro::html;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

pub async fn start() -> Result<(), JsValue> {
  let ctx = Context::new();
  let counter = ctx.create_signal(0);
  let p = html! {
    p id="test" class="test" {
      "Counter is "
      span id="counter" { (&counter.get().to_string()) }
    }
  };
  body()?.append_child(&p)?;
  let btn = html! {
    button { "+" }
  };
  body()?.append_child(&btn)?;
  {
    let counter = counter.clone();
    let p = p.dyn_into::<HtmlElement>()?;
    ctx.create_effect(move || {
      document()
        .unwrap()
        .get_element_by_id("counter")
        .unwrap()
        .set_inner_html(counter.get().to_string().as_str());
    });
  }
  {
    let counter = counter.clone();
    let cl = Closure::wrap(Box::new(move || {
      let new_val = *counter.get() + 1;
      counter.set(new_val);
    }) as Box<dyn FnMut()>);
    btn.add_event_listener_with_callback("click", cl.as_ref().unchecked_ref())?;
    cl.forget();
  }
  Ok(())
}
