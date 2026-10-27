use fluid_macro::html;

fn main() {
  let on = std::rc::Rc::new(fluid::Signal::new(true));
  let _ = html! { div class={ move || "a" } class:b={ true } {} };
}
