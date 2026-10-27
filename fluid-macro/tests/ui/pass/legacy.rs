// The syntax fluid supported before the roadmap, as its README shows it.
use fluid::Context;
use fluid_macro::html;
use wasm_bindgen::prelude::*;

fn build() -> Result<web_sys::Element, JsValue> {
  let ctx = Context::new();
  let counter = ctx.create_signal(0);
  let label = String::from("Counter");
  let class = "even";
  let name = ctx.create_signal(String::from("n"));
  let shared = std::rc::Rc::new(String::from("s"));
  let p = html! {
    div {
      style { r#".odd { color: red; }"# }
      p
      id="test"
      class=#{ |counter| if *counter.get() % 2 == 0 { "even" } else { "odd" } }
      title={ &label }
      data={ class }
      name={ &name.get() }
      lang={ &shared }
      dir=#{ |name| &name.get() }
      {
        "Counter"
        { " is: " }
        { &label }
        { &name.get() }
        { &shared }
        #{ |counter| counter.get().to_string() }
        #{ |counter| &format!("<b>{}</b>", counter.get()) }
      }
      button
      @click={ move |_| {
        let new_val = *counter.get() + 1;
        counter.set(new_val);
      }}
      { "+" }
    }
  };
  let _text: web_sys::Text = html! { "root text" };
  let _text: web_sys::Text = html! { { "root expr" } };
  Ok(p)
}

fn main() {
  let _ = build;
}
