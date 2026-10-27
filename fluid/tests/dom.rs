//! DOM behaviour of `html!` and `fluid::dom`, run headless:
//! `cargo test -p fluid --target wasm32-unknown-unknown` (see README).
#![cfg(target_arch = "wasm32")]

use std::cell::Cell;
use std::rc::Rc;

use fluid::{Context, IntoNode, ListenerOptions, Signal};
use fluid_macro::html;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::{Element, Event, EventInit, HtmlInputElement, MouseEvent, MouseEventInit, Node};

wasm_bindgen_test_configure!(run_in_browser);

fn signal<T>(value: T) -> Rc<Signal<T>> {
  Rc::new(Signal::new(value))
}

fn click(element: &Element) -> MouseEvent {
  let init = MouseEventInit::new();
  init.set_bubbles(true);
  init.set_cancelable(true);
  init.set_client_x(7);
  let event = MouseEvent::new_with_mouse_event_init_dict("click", &init).unwrap();
  element.dispatch_event(&event).unwrap();
  event
}

#[wasm_bindgen_test]
fn legacy_syntax_renders_as_before() {
  let ctx = Context::new();
  let counter = ctx.create_signal(1);
  let p = html! {
    p
    id="test"
    class=#{ |counter| if *counter.get() % 2 == 0 { "even" } else { "odd" } }
    {
      "Counter"
      { " is: " }
      #{ |counter| format!("<b>{}</b>", counter.get()) }
    }
  };
  assert_eq!(
    p.outer_html(),
    r#"<p id="test" class="odd">Counter is: <span><b>1</b></span></p>"#
  );
  counter.set(2);
  assert_eq!(
    p.outer_html(),
    r#"<p id="test" class="even">Counter is: <span><b>2</b></span></p>"#
  );
}

#[wasm_bindgen_test]
fn attributes_keep_source_order_and_any_name() {
  let el = html! { input type="text" aria-label="Name" data-step-2={ 2 } "x:y"="z" for="a" {} };
  let names: Vec<String> = el
    .get_attribute_names()
    .iter()
    .map(|name| name.as_string().unwrap())
    .collect();
  assert_eq!(names, ["type", "aria-label", "data-step-2", "x:y", "for"]);
  assert_eq!(el.get_attribute("data-step-2").as_deref(), Some("2"));
}

struct Badge(&'static str);
impl IntoNode for Badge {
  fn into_node(self) -> Node {
    html! { b { { self.0 } } }.into()
  }
}

#[wasm_bindgen_test]
fn children_can_be_nodes_text_or_into_node() {
  let inner = html! { em { "x" } };
  let none: Option<Element> = None;
  let el = html! {
    div {
      { inner }
      { 42 }
      { "<i>escaped</i>" }
      { Badge("b") }
      { none }
      { vec![html! { i {} }, html! { u {} }] }
    }
  };
  assert_eq!(
    el.inner_html(),
    "<em>x</em>42&lt;i&gt;escaped&lt;/i&gt;<b>b</b><i></i><u></u>"
  );
}

#[wasm_bindgen_test]
fn reactive_text_updates_one_text_node() {
  let count = signal(1);
  let el = html! { p { "n=" #text{ |count| format!("<{}>", count.value()) } } };
  let text = el.last_child().unwrap();
  assert_eq!(el.inner_html(), "n=&lt;1&gt;");
  count.set(2);
  assert_eq!(el.inner_html(), "n=&lt;2&gt;");
  assert!(
    el.last_child().unwrap().is_same_node(Some(&text)),
    "updated in place"
  );
}

#[wasm_bindgen_test]
fn closure_children_switch_between_text_and_nodes() {
  let show = signal(false);
  let el = {
    let show = show.clone();
    html! {
      div {
        "a"
        { move || if show.value() { Some(html! { b { "on" } }) } else { None } }
        "z"
      }
    }
  };
  assert_eq!(el.inner_html(), "az");
  show.set(true);
  assert_eq!(el.inner_html(), "a<b>on</b>z");
  show.set(false);
  assert_eq!(el.inner_html(), "az");
}

#[wasm_bindgen_test]
fn bool_option_property_and_class_toggles() {
  let ctx = Context::new();
  let on = ctx.create_signal(false);
  let el = html! {
    input
    type="checkbox"
    class="base"
    class:"is-on also"=#{ |on| on.value() }
    disabled=#{ |on| !on.value() }
    title={ None::<&str> }
    prop:checked=#{ |on| on.value() }
    {}
  };
  let input: &HtmlInputElement = el.unchecked_ref();
  assert_eq!(el.get_attribute("class").as_deref(), Some("base"));
  assert!(el.has_attribute("disabled"));
  assert!(!el.has_attribute("title"));
  assert!(!input.checked());
  on.set(true);
  assert_eq!(
    el.get_attribute("class").as_deref(),
    Some("base is-on also")
  );
  assert!(!el.has_attribute("disabled"));
  assert!(input.checked());
}

#[wasm_bindgen_test]
fn svg_elements_use_the_svg_namespace() {
  const SVG: &str = "http://www.w3.org/2000/svg";
  let el = html! {
    svg viewBox="0 0 10 10" {
      line x1="0" {}
      foreignObject { div {} }
    }
  };
  let line = el.first_element_child().unwrap();
  let div = el
    .last_element_child()
    .unwrap()
    .first_element_child()
    .unwrap();
  assert_eq!(el.namespace_uri().as_deref(), Some(SVG));
  assert_eq!(line.namespace_uri().as_deref(), Some(SVG));
  assert_eq!(
    div.namespace_uri().as_deref(),
    Some("http://www.w3.org/1999/xhtml")
  );
  let path = html! { path d="M0 0" {} };
  assert_eq!(path.namespace_uri().as_deref(), Some(SVG));
}

#[wasm_bindgen_test]
fn typed_events_and_modifiers() {
  let x = Rc::new(Cell::new(0));
  let el = {
    let x = x.clone();
    html! { button @click.prevent={ move |e: MouseEvent| x.set(e.client_x()) } {} }
  };
  let event = click(&el);
  assert_eq!(x.get(), 7);
  assert!(event.default_prevented());
}

#[wasm_bindgen_test]
fn listeners_are_removed_with_their_scope() {
  let clicks = Rc::new(Cell::new(0));
  let ctx = Context::new();
  let el = ctx.run(|| {
    let clicks = clicks.clone();
    html! { button @click={ move |_| clicks.set(clicks.get() + 1) } {} }
  });
  click(&el);
  ctx.dispose();
  click(&el);
  assert_eq!(clicks.get(), 1);
}

#[wasm_bindgen_test]
fn listen_returns_a_removable_listener() {
  let el = html! { div {} };
  let seen = Rc::new(Cell::new(0));
  let ping = || {
    let init = EventInit::new();
    el.dispatch_event(&Event::new_with_event_init_dict("ping", &init).unwrap())
      .unwrap();
  };
  let listener = {
    let seen = seen.clone();
    fluid::listen(
      &el,
      "ping",
      ListenerOptions::new().once(),
      move |_: Event| seen.set(seen.get() + 1),
    )
  };
  ping();
  ping();
  assert_eq!(seen.get(), 1, "once");
  drop(listener);
  let listener = {
    let seen = seen.clone();
    fluid::listen(&el, "ping", ListenerOptions::new(), move |_: Event| {
      seen.set(seen.get() + 1)
    })
  };
  ping();
  listener.remove();
  ping();
  assert_eq!(seen.get(), 2);
}
