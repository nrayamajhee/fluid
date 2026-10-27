use fluid_macro::html;

fn main() {
  let _ = html! { div { #markdown{ |x| x } } };
}
