//! Component tests: the app is mounted in a headless browser and driven the
//! way a user would. These are fluid's tests; run them with
//! `cargo test -p fluid-example --target wasm32-unknown-unknown`.

use wasm_bindgen_test::*;
use web_sys::{EventInit, HtmlElement};

use super::*;

wasm_bindgen_test_configure!(run_in_browser);

/// The app mounted in the page; dropping it disposes its scope and removes
/// it.
struct App {
  ctx: Context,
  root: Element,
}

impl Drop for App {
  fn drop(&mut self) {
    self.ctx.dispose();
    self.root.remove();
  }
}

impl App {
  fn mount() -> Self {
    let ctx = Context::new();
    ctx.provide_context(Rc::new(Store::default()));
    let root = ctx.run(|| app(&ctx));
    body().append_child(&root).unwrap();
    Self { ctx, root }
  }

  fn find(&self, selector: &str) -> Element {
    self
      .root
      .query_selector(selector)
      .unwrap()
      .unwrap_or_else(|| panic!("nothing matches `{selector}`"))
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

  fn titles(&self) -> Vec<String> {
    let titles = self.root.query_selector_all("li span").unwrap();
    (0..titles.length())
      .filter_map(|index| titles.get(index)?.text_content())
      .collect()
  }

  /// Types a title and submits the form with the Add button.
  fn add(&self, title: &str) {
    let input: HtmlInputElement = self.find(INPUT).unchecked_into();
    input.set_value(title);
    let typed = Event::new_with_event_init_dict("input", &EventInit::new()).unwrap();
    input.dispatch_event(&typed).unwrap();
    self.click(ADD);
  }
}

const INPUT: &str = "input[aria-label='New todo']";
const ADD: &str = "button[type=submit]";
const LEFT: &str = "footer span";
const CLEAR: &str = "footer > button";

#[wasm_bindgen_test]
fn adding_completing_and_removing() {
  let app = App::mount();
  assert_eq!(app.text("p"), "Nothing to do yet.");
  assert!(app.disabled(ADD), "nothing typed yet");
  assert!(app.disabled(CLEAR));

  app.add("   ");
  assert!(app.disabled(ADD), "blank titles can't be added");
  app.add("milk");
  app.add("eggs");
  assert_eq!(app.titles(), ["milk", "eggs"]);
  let input: HtmlInputElement = app.find(INPUT).unchecked_into();
  assert_eq!(input.value(), "", "adding clears the draft");
  assert_eq!(app.text(LEFT), "2 left");

  let init = EventInit::new();
  init.set_cancelable(true);
  let submit = Event::new_with_event_init_dict("submit", &init).unwrap();
  assert!(
    !app.find("form").dispatch_event(&submit).unwrap(),
    "@submit.prevent keeps the page from reloading"
  );
  assert_eq!(
    app.titles(),
    ["milk", "eggs"],
    "an empty draft adds nothing"
  );

  let eggs = app.find("li:nth-child(2)");
  app.click("li:nth-child(1) input");
  assert!(app
    .find("li:nth-child(1)")
    .class_name()
    .contains("line-through"));
  assert!(!eggs.class_name().contains("line-through"));
  assert_eq!(app.text(LEFT), "1 left");
  assert!(!app.disabled(CLEAR));

  app.click(CLEAR);
  assert_eq!(app.titles(), ["eggs"]);
  assert!(
    app.find("li").is_same_node(Some(&eggs)),
    "keyed rows are kept, not rebuilt"
  );

  app.click("button[aria-label=Remove]");
  assert!(app.titles().is_empty());
  assert_eq!(app.text("p"), "Nothing to do yet.");
}

#[wasm_bindgen_test]
fn filters_show_matching_todos() {
  let app = App::mount();
  app.add("milk");
  app.add("eggs");
  app.click("li:nth-child(1) input");

  let (all, active, done) = (
    "nav button:nth-child(1)",
    "nav button:nth-child(2)",
    "nav button:nth-child(3)",
  );
  let pressed = |button: &str| app.find(button).get_attribute("aria-pressed");
  let highlighted = |button: &str| app.find(button).class_name().contains("bg-slate-900");
  assert_eq!(pressed(all).as_deref(), Some("true"));

  app.click(active);
  assert_eq!(app.titles(), ["eggs"]);
  assert!(highlighted(active) && !highlighted(all));
  assert_eq!(pressed(active).as_deref(), Some("true"));
  assert_eq!(pressed(all).as_deref(), Some("false"));

  app.click(done);
  assert_eq!(app.titles(), ["milk"]);
  app.click("li input");
  assert!(app.titles().is_empty(), "un-done todos leave the Done view");
  assert_eq!(app.text("p"), "Nothing done yet.");

  app.click(active);
  assert_eq!(app.titles(), ["milk", "eggs"]);
  app.click("li:nth-child(1) input");
  app.click("li input");
  assert_eq!(app.text("p"), "Nothing left to do.");
  assert_eq!(app.text(LEFT), "0 left");

  app.click(all);
  assert_eq!(app.titles(), ["milk", "eggs"]);
}
