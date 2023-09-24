use fluid::Context;
use fluid_macro::html;
use gloo_utils::body;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

pub async fn async_main() -> Result<(), JsValue> {
  let ctx = Context::new();
  let counter = ctx.create_signal(0);
  {
    let c1 = counter.clone();
    let c2 = counter.clone();
    let c3 = counter.clone();
    let p = html! {
      div {
        p
        id="test"
        class=[ctx, &c1.get().to_string()]
        {
          "Counter"
          ( " is " )
          [ctx, &c2.get().to_string()]
        }
        button
        @click=(move |_| {
          let new_val = *c3.get() + 1;
          counter.set(new_val);
        })
        { "+" }
      }
    };
    body().append_child(&p)?;
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
