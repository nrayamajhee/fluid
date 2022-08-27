use fluid::{body, Context};
use fluid_macro::html;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

pub async fn start() -> Result<(), JsValue> {
  let p = html! {
    p id="test" class="test" {
      // (counter.get())
      "Hello "
      span { span { "World!" } }
      br {}
      "Hey "
      span { "There!" }
    }
  };
  let btn = html! {
    button {}
  };
  btn.set_inner_html("+");
  let ctx = Context::new();
  let counter = ctx.create_signal(0);
  body()?.append_child(&p)?;
  let p = html! { p {} };
  body()?.append_child(&p)?;
  {
    let counter = counter.clone();
    let p = p.dyn_into::<HtmlElement>()?;
    ctx.create_effect(move || {
      p.set_inner_text(counter.get().to_string().as_str());
    });
  }
  body()?.append_child(&btn)?;
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
