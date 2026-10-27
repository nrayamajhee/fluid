use fluid_macro::html;

fn main() {
  let _ = html! { if true { "root" } };
  let _ = html! { ul { for item in [1, 2] { li {} } } };
  let _ = html! { div { if true { "a" } else if |x| true { "b" } } };
}
