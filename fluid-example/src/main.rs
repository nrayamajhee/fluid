use fluid::{js_closure, Context};
use gloo_utils::{body};
use fluid_macro::html;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

pub async fn async_main() -> Result<(), JsValue> {
  let ctx = Context::new();
  let counter = ctx.create_signal(0);
  {
    let c1 = counter.clone();
    let c2 = counter.clone();
    let p = html! {
      p id="test" class=[ctx, &c1.get().to_string()] {
        "Counter"
        ( " is " )
        [ctx, &c2.get().to_string()]
      }
    };
    body().append_child(&p)?;
  }
  let btn = html! {
    button { "+" }
  };
  body().append_child(&btn)?;
  {
    let counter = counter.clone();
    let cl = js_closure(move |_| {
      let new_val = *counter.get() + 1;
      counter.set(new_val);
    });
    btn.add_event_listener_with_callback("click", cl.as_ref().unchecked_ref())?;
    cl.forget();
  }
  Ok(())
}

fn main() {
  wasm_bindgen_futures::spawn_local(async move {
    async_main().await.unwrap_or_else(|err| {
      gloo_console::log!("Couldn't spawn async main", err);
    })
  })
}
