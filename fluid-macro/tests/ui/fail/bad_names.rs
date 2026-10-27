use fluid_macro::html;

fn main() {
  let _ = html! { div "bad name"="x" {} };
}
