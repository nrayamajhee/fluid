use fluid_macro::html;

struct Opaque;

fn main() {
  let _ = html! { div { { Opaque } } };
}
