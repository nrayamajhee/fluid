use std::cell::Cell;
use std::rc::Rc;

use fluid::{batch, Context, Signal};
use fluid_macro::html;
use gloo_utils::body;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, Event, HtmlInputElement, KeyboardEvent, MouseEvent};

#[derive(Clone)]
struct Todo {
  id: u32,
  title: String,
  done: Rc<Signal<bool>>,
}

/// Shared state, provided once as a context value instead of passed around.
#[derive(Default)]
struct Store {
  todos: Signal<Vec<Todo>>,
  next_id: Cell<u32>,
}

impl Store {
  fn add(&self, title: String) {
    let id = self.next_id.replace(self.next_id.get() + 1);
    let done = Rc::new(Signal::new(false));
    self
      .todos
      .update(|todos| todos.push(Todo { id, title, done }));
  }
  fn remove(&self, id: u32) {
    self
      .todos
      .update(|todos| todos.retain(|todo| todo.id != id));
  }
}

/// The original counter, in today's syntax: `#{ |signals| .. }` blocks.
fn counter(ctx: &Context) -> Element {
  let counter = ctx.create_signal(0);
  let doubled = ctx.create_memo({
    let counter = counter.clone();
    move || counter.value() * 2
  });
  html! {
    section {
      style { r#"
        .odd { color: red; }
        .even { color: blue; }
        .done { text-decoration: line-through; opacity: 0.6; }
      "#}
      h2 { "Counter" }
      p
      id="test"
      class=#{ |counter| if *counter.get() % 2 == 0 { "even" } else { "odd" } }
      {
        "Counter"
        { " is: " }
        #text{ |counter| counter.value() }
        ", doubled: "
        #text{ |doubled| doubled.value() }
      }
      button
      aria-label="Increment (shift: by ten)"
      @click={ move |e: MouseEvent| counter.update(|n| *n += if e.shift_key() { 10 } else { 1 }) }
      { "+" }
    }
  }
}

fn input_value(event: &Event) -> String {
  event
    .target()
    .and_then(|target| target.dyn_into::<HtmlInputElement>().ok())
    .map(|input| input.value())
    .unwrap_or_default()
}

fn check_icon() -> Element {
  html! {
    svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true" {
      path d="M3 8.5l3 3 7-7" fill="none" stroke="currentColor" stroke-width="2" {}
    }
  }
}

fn todos(ctx: &Context) -> Element {
  let store = ctx
    .use_context::<Rc<Store>>()
    .expect("main provides the store");
  let draft = ctx.create_signal(String::new());
  let remaining = ctx.create_memo({
    let store = store.clone();
    move || {
      store
        .todos
        .with(|todos| todos.iter().filter(|todo| !todo.done.value()).count())
    }
  });
  let add = {
    let (store, draft) = (store.clone(), draft.clone());
    move || {
      let title = draft.cloned();
      if !title.trim().is_empty() {
        // One update for both writes.
        batch(|| {
          store.add(title);
          draft.set(String::new());
        });
      }
    }
  };
  html! {
    section {
      h2 { { check_icon() } " Todos" }
      input
      type="text"
      placeholder="What needs doing?"
      prop:value=#{ |draft| draft.cloned() }
      @input={ { let draft = draft.clone(); move |e: Event| draft.set(input_value(&e)) } }
      @keydown={ move |e: KeyboardEvent| if e.key() == "Enter" { add() } }
      {}
      ul {
        for |store| todo in store.todos.cloned(), key = todo.id {
          li class:done=#{ |todo| todo.done.value() } {
            input
            type="checkbox"
            prop:checked=#{ |todo| todo.done.value() }
            @change={ { let done = todo.done.clone(); move |_| done.update(|done| *done = !*done) } }
            {}
            { todo.title.clone() }
            " "
            button aria-label="Remove" @click={ move |_| store.remove(todo.id) } { "×" }
          }
        }
      }
      if |remaining| remaining.value() == 0 {
        p { "Nothing left to do." }
      } else {
        p { #text{ |remaining| remaining.value() } " left" }
      }
    }
  }
}

pub async fn async_main() -> Result<(), JsValue> {
  let ctx = Context::new();
  ctx.provide_context(Rc::new(Store::default()));
  // Built inside `run`, every effect and listener belongs to `ctx`, so
  // `ctx.dispose()` would tear the whole page down.
  let app = ctx.run(|| html! { main { { counter(&ctx) } { todos(&ctx) } } });
  body().append_child(&app)?;
  Ok(())
}

fn main() {
  wasm_bindgen_futures::spawn_local(async move {
    async_main().await.unwrap_or_else(|err| {
      gloo_console::log!("Couldn't spawn async main", err);
    })
  })
}
