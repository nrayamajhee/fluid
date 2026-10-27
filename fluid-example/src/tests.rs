//! Component tests: each section of the example is mounted on its own in a
//! headless browser and driven the way a user would. These are fluid's
//! tests; run them with `cargo test -p fluid-example --target
//! wasm32-unknown-unknown` (see the README).

use std::rc::Rc;

use gloo_timers::future::TimeoutFuture;
use wasm_bindgen_test::*;
use web_sys::{
  EventInit, EventTarget, HtmlElement, KeyboardEventInit, MouseEventInit, WheelEventInit,
};

use super::*;

wasm_bindgen_test_configure!(run_in_browser);

/// A section mounted in the page; dropping it disposes its scope and
/// removes it.
struct Mounted {
  ctx: Context,
  root: Element,
}

impl Drop for Mounted {
  fn drop(&mut self) {
    self.ctx.dispose();
    self.root.remove();
  }
}

fn mount(build: impl FnOnce(&Context) -> Element) -> Mounted {
  let ctx = Context::new();
  ctx.provide_context(Rc::new(Store::default()));
  let root = ctx.run(|| build(&ctx));
  body().append_child(&root).unwrap();
  Mounted { ctx, root }
}

impl Mounted {
  fn find(&self, selector: &str) -> Element {
    self
      .root
      .query_selector(selector)
      .unwrap()
      .unwrap_or_else(|| panic!("nothing matches `{selector}`"))
  }

  fn all(&self, selector: &str) -> Vec<Element> {
    let found = self.root.query_selector_all(selector).unwrap();
    (0..found.length())
      .filter_map(|index| found.get(index))
      .map(|node| node.unchecked_into())
      .collect()
  }

  fn text(&self, selector: &str) -> String {
    self.find(selector).text_content().unwrap_or_default()
  }

  fn click(&self, selector: &str) {
    self.find(selector).unchecked_into::<HtmlElement>().click();
  }

  fn disabled(&self, selector: &str) -> bool {
    self.find(selector).has_attribute("disabled")
  }
}

/// Dispatches a keydown, returning whether a listener prevented its default.
fn key(target: &EventTarget, key: &str) -> bool {
  let init = KeyboardEventInit::new();
  init.set_key(key);
  init.set_bubbles(true);
  init.set_cancelable(true);
  let event = KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init).unwrap();
  !target.dispatch_event(&event).unwrap()
}

#[wasm_bindgen_test]
fn counter_updates_text_class_and_memo() {
  let counter = mount(counter);
  assert_eq!(counter.text("#test"), "Counter is: 0, doubled: 0");
  assert!(counter.find("#test").class_name().contains("text-blue-600"));
  assert!(
    counter.root.query_selector("#test span").unwrap().is_none(),
    "#text renders text nodes, not spans"
  );

  counter.click("button");
  assert_eq!(counter.text("#test"), "Counter is: 1, doubled: 2");
  assert!(counter.find("#test").class_name().contains("text-rose-600"));

  let init = MouseEventInit::new();
  init.set_shift_key(true);
  let shift_click = MouseEvent::new_with_mouse_event_init_dict("click", &init).unwrap();
  counter.find("button").dispatch_event(&shift_click).unwrap();
  assert_eq!(counter.text("#test"), "Counter is: 11, doubled: 22");
}

#[wasm_bindgen_test]
fn todos_add_toggle_reorder_and_clear() {
  let todos = mount(todos);
  let status = || todos.text(":scope > div > p");
  let titles = || {
    todos
      .all("li span")
      .iter()
      .map(|title| title.text_content().unwrap_or_default())
      .collect::<Vec<_>>()
  };
  let clear = ":scope > div > button";
  assert_eq!(status(), "Nothing yet.");
  assert!(todos.disabled(clear));

  let input: HtmlInputElement = todos.find("input[type=text]").unchecked_into();
  for title in ["milk", "eggs", "  "] {
    input.set_value(title);
    input
      .dispatch_event(&Event::new_with_event_init_dict("input", &EventInit::new()).unwrap())
      .unwrap();
    key(&input, "Enter");
  }
  assert_eq!(titles(), ["milk", "eggs"], "blank titles are skipped");
  assert_eq!(input.value(), "  ", "only added titles clear the draft");
  assert_eq!(status(), "2 left");

  let eggs = todos.find("li:nth-child(2)");
  todos.click("li:nth-child(1) input");
  assert!(todos
    .find("li:nth-child(1)")
    .class_name()
    .contains("line-through"));
  assert!(!eggs.class_name().contains("line-through"));
  assert_eq!(status(), "1 left");
  assert!(!todos.disabled(clear));

  todos.click(clear);
  assert_eq!(titles(), ["eggs"]);
  assert!(
    todos.find("li").is_same_node(Some(&eggs)),
    "keyed rows are kept, not rebuilt"
  );
  assert!(todos.disabled(clear));

  todos.click("li input");
  assert_eq!(status(), "All done!");
  todos.click("button[aria-label=Remove]");
  assert!(titles().is_empty());
  assert_eq!(status(), "Nothing yet.");
}

#[wasm_bindgen_test]
fn level_meter_follows_wheel_keys_and_buttons() {
  let level = mount(level_meter);
  let meter = level.find("[role=slider]");
  let now = || meter.get_attribute("aria-valuenow").unwrap();
  let bar = || {
    level
      .find("rect:nth-of-type(2)")
      .get_attribute("width")
      .unwrap()
  };
  let status = || level.text(":scope > p:nth-of-type(2)");
  assert_eq!((now().as_str(), bar().as_str()), ("5", "100"));
  assert_eq!(status(), "Quiet (5)");
  assert!(!meter.has_attribute("title"));
  assert_eq!(
    level.find("rect").namespace_uri().as_deref(),
    Some("http://www.w3.org/2000/svg")
  );

  let init = WheelEventInit::new();
  init.set_delta_y(-100.);
  init.set_cancelable(true);
  let wheel = WheelEvent::new_with_event_init_dict("wheel", &init).unwrap();
  let not_prevented = meter.dispatch_event(&wheel).unwrap();
  assert!(!not_prevented, ".prevent calls preventDefault");
  assert_eq!(now(), "6");
  assert_eq!(status(), "Loud (6)");

  for _ in 0..6 {
    assert!(
      key(&window(), "ArrowUp"),
      "the window listener takes the arrows"
    );
  }
  assert_eq!((now().as_str(), bar().as_str()), ("10", "200"), "clamped");
  assert_eq!(status(), "Maximum!");
  assert_eq!(
    meter.get_attribute("title").as_deref(),
    Some("That's the maximum")
  );
  assert!(level.disabled("[aria-label=Louder]"));

  for _ in 0..12 {
    level.click("[aria-label=Quieter]");
  }
  assert_eq!((now().as_str(), bar().as_str()), ("0", "0"));
  assert_eq!(status(), "Silent");
  assert!(!meter.has_attribute("title"));
  assert!(level.disabled("[aria-label=Quieter]"));

  drop(level);
  assert!(
    !key(&window(), "ArrowUp"),
    "the window listener is removed with its scope"
  );
}

#[wasm_bindgen_test]
async fn hidden_branches_are_disposed_and_rebuilt_fresh() {
  let scopes = mount(scopes);
  let running = || scopes.text("p");
  TimeoutFuture::new(350).await;
  assert_ne!(running(), "Running for 0.0 s", "the stopwatch ticks");
  assert_eq!(scopes.text("p:nth-of-type(2)"), "Shown 1 time(s).");

  scopes.click("input");
  assert_eq!(
    running(),
    "Hidden: its interval was cleared and its effects disposed."
  );
  scopes.click("input");
  assert_eq!(running(), "Running for 0.0 s", "a fresh stopwatch");
  assert_eq!(scopes.text("p:nth-of-type(2)"), "Shown 2 time(s).");
}

#[wasm_bindgen_test]
fn children_render_components_text_and_closures() {
  let children = mount(|_| children());
  assert_eq!(children.all("span.rounded-full").len(), 3);
  assert_eq!(
    children.text("p:nth-of-type(2)"),
    "Numbers: 42, 1.5 (an Option child)"
  );
  assert_eq!(children.text("code"), "<b>not bold</b>");
  assert!(
    children.root.query_selector("code b").unwrap().is_none(),
    "escaped"
  );
  assert_eq!(children.text("p:nth-of-type(4) b"), "bold", "#html is raw");

  let closure = "p:nth-of-type(5)";
  assert!(children
    .root
    .query_selector(&format!("{closure} b"))
    .unwrap()
    .is_none());
  children.click(&format!("{closure} button"));
  assert_eq!(children.text(&format!("{closure} b")), "now an element");
  children.click(&format!("{closure} button"));
  assert!(children
    .root
    .query_selector(&format!("{closure} b"))
    .unwrap()
    .is_none());
}
