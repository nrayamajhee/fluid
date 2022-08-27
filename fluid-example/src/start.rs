// use fluid::Context;
use fluid_macro::html;
use wasm_bindgen::JsValue;

pub async fn start() -> Result<(), JsValue> {
  let document = web_sys::window()
    .ok_or(JsValue::from("No Window"))?
    .document()
    .ok_or(JsValue::from("No Document"))?;
  let p = html! {
      p id="test" class="test" {
          span { "Hello" }
          // This fails for now
          // span { "Hello" }
      }
  };
  p.set_inner_html("Hello WASM!");
  let body = document.body().ok_or(JsValue::from("No Body"))?;
  body.append_child(&p)?;
  Ok(())
}

// use std::rc::Rc;
//
// fn main() {
//     let ctx = Context::new();
//     let a = ctx.create_signal("Hello");
//     let a = Rc::new(a);
//     {
//         let a = a.clone();
//         ctx.create_effect(move || {
//             println!("Effect 1: {}", a.get());
//         });
//     }
//     ctx.create_effect(move || {
//         println!("Effect Mid");
//     });
//     {
//         let a = a.clone();
//         ctx.create_effect(move || {
//             println!("Effect 2: {}", a.get());
//         });
//     }
//     a.set("Hi");
//     a.set("There!");
// }
