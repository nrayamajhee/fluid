//! A todo list with filters: one small app that uses fluid's main features.

use std::cell::Cell;
use std::rc::Rc;

use fluid::{batch, Context, Signal};
use fluid_macro::html;
use gloo_utils::body;
use wasm_bindgen::JsCast;
use web_sys::{Element, Event, HtmlInputElement};

// Tailwind (built by Trunk from `src/tailwind.css`) scans this file for
// class names, so every class is written out whole.
const BUTTON: &str = "rounded-md border border-slate-300 bg-white px-3 py-1 text-sm font-medium hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-40";

#[derive(Clone)]
struct Todo {
  id: u32,
  title: String,
  done: Rc<Signal<bool>>,
}

#[derive(Clone, Copy, PartialEq)]
enum Filter {
  All,
  Active,
  Done,
}

impl Filter {
  fn shows(self, todo: &Todo) -> bool {
    match self {
      Self::All => true,
      Self::Active => !todo.done.value(),
      Self::Done => todo.done.value(),
    }
  }

  fn label(self) -> &'static str {
    match self {
      Self::All => "All",
      Self::Active => "Active",
      Self::Done => "Done",
    }
  }
}

/// The app's state, provided as a context value instead of passed around.
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

  fn clear_done(&self) {
    self
      .todos
      .update(|todos| todos.retain(|todo| !todo.done.value()));
  }

  fn visible(&self, filter: Filter) -> Vec<Todo> {
    self.todos.with(|todos| {
      todos
        .iter()
        .filter(|todo| filter.shows(todo))
        .cloned()
        .collect()
    })
  }
}

fn check_icon() -> Element {
  html! {
    svg class="size-7 text-emerald-600" viewBox="0 0 16 16" aria-hidden="true" {
      path d="M3 8.5l3 3 7-7" fill="none" stroke="currentColor" stroke-width="2" {}
    }
  }
}

/// A component is a function returning an element.
fn todo_item(todo: Todo) -> Element {
  let store = fluid::use_context::<Rc<Store>>().expect("app provides the store");
  let done = todo.done.clone();
  html! {
    li
    class="flex items-center gap-3 py-2"
    class:"text-slate-400 line-through"=#{ |done| done.value() }
    {
      input
      type="checkbox"
      class="size-4 accent-emerald-600"
      aria-label="Done"
      prop:checked=#{ |done| done.value() }
      @change={ move |_| done.update(|done| *done = !*done) }
      {}
      span class="flex-1" { { todo.title } }
      button
      class="rounded px-2 text-slate-400 hover:text-rose-600"
      aria-label="Remove"
      @click={ move |_| store.remove(todo.id) }
      { "×" }
    }
  }
}

fn filter_button(filter: &Rc<Signal<Filter>>, value: Filter) -> Element {
  let (selected, pressed, set) = (filter.clone(), filter.clone(), filter.clone());
  html! {
    button
    class="rounded-md px-2.5 py-1"
    class:"bg-slate-900 text-white"=#{ |selected| selected.value() == value }
    aria-pressed={ move || (pressed.value() == value).to_string() }
    @click={ move |_| set.set(value) }
    { { value.label() } }
  }
}

fn app(ctx: &Context) -> Element {
  let store = ctx
    .use_context::<Rc<Store>>()
    .expect("main provides the store");
  let draft = ctx.create_signal(String::new());
  let filter = ctx.create_signal(Filter::All);
  let remaining = ctx.create_memo({
    let store = store.clone();
    move || store.visible(Filter::Active).len()
  });
  let add = {
    let (store, draft) = (store.clone(), draft.clone());
    move |_| {
      let title = draft.cloned().trim().to_string();
      if title.is_empty() {
        return;
      }
      // Both writes, one update.
      batch(|| {
        store.add(title);
        draft.set(String::new());
      });
    }
  };
  let typed = draft.clone();
  html! {
    main class="mx-auto mt-12 max-w-md space-y-4 rounded-xl bg-white p-6 shadow-sm" {
      h1 class="flex items-center gap-2 text-2xl font-bold" { { check_icon() } "Todos" }
      form class="flex gap-2" @submit.prevent={ add } {
        input
        class="flex-1 rounded-md border border-slate-300 px-3 py-1.5 focus:border-blue-500 focus:outline-none"
        aria-label="New todo"
        placeholder="What needs doing?"
        prop:value=#{ |draft| draft.cloned() }
        @input={ move |e: Event| typed.set(e.target().unwrap().unchecked_into::<HtmlInputElement>().value()) }
        {}
        button
        type="submit"
        class={ BUTTON }
        disabled=#{ |draft| draft.with(|draft| draft.trim().is_empty()) }
        { "Add" }
      }
      if |store, filter| store.visible(filter.value()).is_empty() {
        p class="py-6 text-center text-slate-500" {
          match |filter| filter.value() {
            Filter::All => { "Nothing to do yet." }
            Filter::Active => { "Nothing left to do." }
            Filter::Done => { "Nothing done yet." }
          }
        }
      } else {
        ul class="divide-y divide-slate-100" {
          for |store, filter| todo in store.visible(filter.value()), key = todo.id {
            { todo_item(todo) }
          }
        }
      }
      footer class="flex items-center justify-between text-sm text-slate-600" {
        span { #text{ |remaining| remaining.value() } " left" }
        nav class="flex gap-1" {
          { filter_button(&filter, Filter::All) }
          { filter_button(&filter, Filter::Active) }
          { filter_button(&filter, Filter::Done) }
        }
        button
        class={ BUTTON }
        disabled=#{ |remaining, store| remaining.value() == store.todos.with(Vec::len) }
        @click={ move |_| store.clear_done() }
        { "Clear done" }
      }
    }
  }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests;

fn main() {
  let ctx = Context::new();
  ctx.provide_context(Rc::new(Store::default()));
  // Built inside `run`, everything on the page belongs to `ctx`.
  let page = ctx.run(|| app(&ctx));
  body().append_child(&page).expect("a body to mount in");
}
