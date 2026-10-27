use fluid_macro::html;

fn main() {
  let _ = html! { div @wheel.passive.prevent={ |_| () } {} };
  let _ = html! { div @click.sometimes={ |_| () } {} };
}
