use std::rc::Rc;

use fluid::{IntoNode, Signal};
use fluid_macro::html;
use web_sys::{Element, KeyboardEvent, MouseEvent, Node, WheelEvent};

struct Badge(&'static str);

impl IntoNode for Badge {
  fn into_node(self) -> Node {
    html! { span class="badge" { { self.0 } } }.into()
  }
}

fn child() -> Element {
  html! { em { "child" } }
}

fn build(count: Rc<Signal<i32>>, open: Rc<Signal<bool>>) -> Element {
  let maybe: Option<&str> = None;
  let items = vec![child(), child()];
  let c = count.clone();
  html! {
    section
    aria-label="Counter"
    data-step-2={ 2 }
    "x-weird:name"="ok"
    {
      { child() }
      { Badge("new") }
      { Some(child()) }
      { items }
      { 42 }
      { 1.5 }
      { move || c.value() * 2 }
      #text{ |count| format!("count {}", count.value()) }
      #html{ |count| format!("<i>{}</i>", count.value()) }
      label for="name" { "Name" }
      input
      type="checkbox"
      disabled={ true }
      title={ maybe }
      prop:checked=#{ |open| open.value() }
      prop:value={ "on" }
      {}
      div
      class="pad"
      class:"bg-held shadow-held"=#{ |open| open.value() }
      class:hidden={ false }
      class:active={ { let open = open.clone(); move || open.value() } }
      title={ { let count = count.clone(); move || format!("{}", count.value()) } }
      {}
      button
      @click={ move |e: MouseEvent| { let _ = e.client_x(); } }
      @wheel.nonpassive={ |e: WheelEvent| e.prevent_default() }
      @keydown.prevent.stop={ |e: KeyboardEvent| { let _ = e.key(); } }
      @mousedown.capture.once={ |_| () }
      @custom-event={ |e| { let _: web_sys::Event = e; } }
      { "go" }
      svg width="10" height="10" viewBox="0 0 10 10" {
        line x1="0" y1="0" x2="10" y2="10" stroke="currentColor" stroke-width="2" {}
        foreignObject { div { "html again" } }
      }
    }
  }
}

fn path() -> Element {
  html! { path d="M0 0L1 1" {} }
}

fn main() {
  let _ = (build, path, control_flow);
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
  Idle,
  Playing(u8),
}

fn control_flow(open: Rc<Signal<bool>>, mode: Rc<Signal<Mode>>, items: Rc<Signal<Vec<(u32, String)>>>) -> Element {
  html! {
    div {
      if |open| open.value() { "open" } else { "closed" }
      if |open| !open.value() { "only when closed" }
      match |mode| mode.value() {
        Mode::Idle => { "idle" }
        Mode::Playing(n) => { { n } }
      }
      ul {
        for |items, open| (id, name) in items.cloned(), key = *id {
          li class:open=#{ |open| open.value() } { { id } ": " { name.clone() } }
        }
      }
      svg { if |open| open.value() { circle r="1" {} } }
    }
  }
}
