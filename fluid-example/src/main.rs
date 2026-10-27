//! A tour of fluid. Each section is one function; its doc comment lists the
//! features it shows.

use std::cell::Cell;
use std::rc::Rc;

use fluid::{batch, untrack, Context, IntoNode, ListenerOptions, Signal};
use fluid_macro::html;
use gloo_timers::callback::Interval;
use gloo_utils::{body, window};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, Event, HtmlInputElement, KeyboardEvent, MouseEvent, Node, WheelEvent};

// Tailwind (built by Trunk from `src/tailwind.css`) scans these sources for
// class names, so every class is written out whole, never pieced together.
const SECTION: &str = "rounded-xl border border-slate-200 bg-white p-5 shadow-sm";
const HEADING: &str = "mb-3 flex items-center gap-2 text-lg font-semibold";
const BUTTON: &str = "rounded-md border border-slate-300 bg-white px-3 py-1 text-sm font-medium shadow-xs hover:bg-slate-50 active:bg-slate-100 disabled:cursor-not-allowed disabled:opacity-40";
const NOTE: &str = "mt-2 text-sm text-slate-500";

/// Counter: the original syntax (`class=#{ |signal| .. }`) next to `#text`
/// nodes, a memo and a typed event.
fn counter(ctx: &Context) -> Element {
  let counter = ctx.create_signal(0);
  let doubled = ctx.create_memo({
    let counter = counter.clone();
    move || counter.value() * 2
  });
  html! {
    section class={ SECTION } {
      h2 class={ HEADING } { "Counter" }
      p
      id="test"
      class=#{ |counter| if *counter.get() % 2 == 0 { "mb-3 font-medium text-blue-600" } else { "mb-3 font-medium text-rose-600" } }
      {
        "Counter is: "
        #text{ |counter| counter.value() }
        ", doubled: "
        #text{ |doubled| doubled.value() }
      }
      button
      class={ BUTTON }
      aria-label="Increment (shift: by ten)"
      @click={ move |e: MouseEvent| counter.update(|n| *n += if e.shift_key() { 10 } else { 1 }) }
      { "+" }
      span class={ NOTE } { " Shift-click adds ten." }
    }
  }
}

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
  fn clear_done(&self) {
    self
      .todos
      .update(|todos| todos.retain(|todo| !todo.done.value()));
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
    svg class="size-5 text-emerald-600" viewBox="0 0 16 16" aria-hidden="true" {
      path d="M3 8.5l3 3 7-7" fill="none" stroke="currentColor" stroke-width="2" {}
    }
  }
}

/// Todos: a keyed `for`, `if`/`else`, `use_context`, `batch`, memos,
/// `prop:` and `class:` bindings, a `bool` attribute and an inline SVG icon.
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
        // Both writes, one update.
        batch(|| {
          store.add(title);
          draft.set(String::new());
        });
      }
    }
  };
  html! {
    section class={ SECTION } {
      h2 class={ HEADING } { { check_icon() } "Todos" }
      input
      class="w-full rounded-md border border-slate-300 px-3 py-2 text-sm focus:border-blue-500 focus:ring-2 focus:ring-blue-200 focus:outline-none"
      type="text"
      aria-label="New todo"
      placeholder="What needs doing? (Enter adds)"
      prop:value=#{ |draft| draft.cloned() }
      @input={ { let draft = draft.clone(); move |e: Event| draft.set(input_value(&e)) } }
      @keydown={ move |e: KeyboardEvent| if e.key() == "Enter" { add() } }
      {}
      ul class="my-3 divide-y divide-slate-100" {
        for |store| todo in store.todos.cloned(), key = todo.id {
          li
          class="flex items-center gap-3 py-2"
          class:"text-slate-400 line-through"=#{ |todo| todo.done.value() }
          {
            input
            class="size-4 accent-emerald-600"
            type="checkbox"
            prop:checked=#{ |todo| todo.done.value() }
            @change={ { let done = todo.done.clone(); move |_| done.update(|done| *done = !*done) } }
            {}
            span class="flex-1" { { todo.title.clone() } }
            button
            class="rounded px-2 text-slate-400 hover:bg-rose-50 hover:text-rose-600"
            aria-label="Remove"
            @click={ move |_| store.remove(todo.id) }
            { "×" }
          }
        }
      }
      div class="flex items-center justify-between" {
      if |remaining, store| store.todos.with(Vec::is_empty) {
        p class="text-sm text-slate-500" { "Nothing yet." }
      } else if remaining.value() == 0 {
        p class="text-sm font-medium text-emerald-600" { "All done!" }
      } else {
        p class="text-sm text-slate-600" { #text{ |remaining| remaining.value() } " left" }
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

#[derive(Clone, Copy, PartialEq)]
enum Loudness {
  Silent,
  Quiet(u8),
  Loud(u8),
}

fn loudness(level: u8) -> Loudness {
  match level {
    0 => Loudness::Silent,
    1..=5 => Loudness::Quiet(level),
    _ => Loudness::Loud(level),
  }
}

/// Level: `match` with bindings and guards, `@wheel.nonpassive.prevent`,
/// hyphenated `aria-*` attributes, an `Option` attribute, reactive SVG
/// attributes, `set_if_changed`, and a window listener from `fluid::listen`.
fn level_meter(ctx: &Context) -> Element {
  let level = ctx.create_signal(5u8);
  let set = {
    let level = level.clone();
    move |next: i32| {
      level.set_if_changed(next.clamp(0, 10) as u8);
    }
  };
  {
    let (set, level) = (set.clone(), level.clone());
    // Removed when the scope this runs in (main's `ctx`) is disposed.
    fluid::listen(
      &window(),
      "keydown",
      ListenerOptions::new(),
      move |e: KeyboardEvent| {
        let current = i32::from(level.value());
        match e.key().as_str() {
          "ArrowUp" => set(current + 1),
          "ArrowDown" => set(current - 1),
          _ => {}
        }
      },
    )
    .owned();
  }
  let (down, up) = (set.clone(), set.clone());
  let (level_wheel, level_down, level_up) = (level.clone(), level.clone(), level.clone());
  html! {
    section class={ SECTION } {
      h2 class={ HEADING } { "Level" }
      p class="mb-3 text-sm text-slate-600" { "Scroll over the meter, press ↑/↓ anywhere, or use the buttons." }
      div
      class="inline-block cursor-ns-resize rounded-full text-blue-600 focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-blue-500"
      role="slider"
      tabindex="0"
      aria-label="Level"
      aria-valuemin="0"
      aria-valuemax="10"
      aria-valuenow=#{ |level| level.value() }
      title=#{ |level| (level.value() == 10).then_some("That's the maximum") }
      @wheel.nonpassive.prevent={ move |e: WheelEvent| set(i32::from(level_wheel.value()) - e.delta_y().signum() as i32) }
      {
        svg width="200" height="12" viewBox="0 0 200 12" {
          rect class="fill-slate-200" width="200" height="12" rx="6" {}
          rect width=#{ |level| u32::from(level.value()) * 20 } height="12" rx="6" fill="currentColor" {}
        }
      }
      p class="my-3 text-sm" {
        match |level| loudness(level.value()) {
          Loudness::Silent => { span class="text-slate-400" { "Silent" } }
          Loudness::Quiet(n) => { "Quiet (" { n } ")" }
          Loudness::Loud(n) if n == 10 => { b class="text-rose-600" { "Maximum!" } }
          Loudness::Loud(n) => { "Loud (" { n } ")" }
        }
      }
      div class="flex gap-2" {
      button
      class={ BUTTON }
      aria-label="Quieter"
      disabled=#{ |level| level.value() == 0 }
      @click={ move |_| down(i32::from(level_down.value()) - 1) }
      { "−" }
      button
      class={ BUTTON }
      aria-label="Louder"
      disabled=#{ |level| level.value() == 10 }
      @click={ move |_| up(i32::from(level_up.value()) + 1) }
      { "+" }
      }
    }
  }
}

/// Counts tenths of a second while it is shown; `on_cleanup` stops the
/// interval when its branch is disposed.
fn stopwatch() -> Element {
  let tenths = Rc::new(Signal::new(0u32));
  let interval = {
    let tenths = tenths.clone();
    Interval::new(100, move || tenths.update(|t| *t += 1))
  };
  fluid::on_cleanup(move || drop(interval));
  html! {
    p class="font-mono text-2xl tabular-nums" { "Running for " #text{ |tenths| format!("{:.1}", f64::from(tenths.value()) / 10.) } " s" }
  }
}

/// Scopes: an `if` branch is its own scope. Hiding the stopwatch disposes
/// its effects and runs its cleanup; `untrack` reads without subscribing.
fn scopes(ctx: &Context) -> Element {
  let shown = ctx.create_signal(true);
  let toggles = ctx.create_signal(0u32);
  let toggle = {
    let (shown, toggles) = (shown.clone(), toggles.clone());
    move |_| {
      batch(|| {
        shown.update(|shown| *shown = !*shown);
        toggles.update(|n| *n += 1);
      })
    }
  };
  html! {
    section class={ SECTION } {
      h2 class={ HEADING } { "Scopes" }
      label class="mb-3 flex items-center gap-2 text-sm" {
        input class="size-4 accent-blue-600" type="checkbox" prop:checked=#{ |shown| shown.value() } @change={ toggle } {}
        " Show the stopwatch"
      }
      if |shown, toggles| shown.value() {
        { stopwatch() }
        // Read untracked: this branch isn't rebuilt when `toggles` changes.
        p class={ NOTE } { "Shown " { untrack(|| toggles.value()) / 2 + 1 } " time(s)." }
      } else {
        p class={ NOTE } { "Hidden: its interval was cleared and its effects disposed." }
      }
    }
  }
}

/// A component: any type implementing `IntoNode` can be a child.
struct Badge(&'static str);

impl IntoNode for Badge {
  fn into_node(self) -> Node {
    html! { span class="mr-1.5 rounded-full bg-blue-50 px-2.5 py-0.5 text-sm font-medium text-blue-700" { { self.0 } } }.into()
  }
}

/// Children: elements, `IntoNode` components, `Vec`s and `Option`s of them,
/// numbers and other `Display` values (escaped), `{ move || .. }` closures,
/// and raw HTML only through an explicit `#html`.
fn children() -> Element {
  let tags = ["rust", "wasm", "signals"];
  let note: Option<Element> = Some(html! { em { "(an Option child)" } });
  let nothing: Option<Element> = None;
  let loud = Rc::new(Signal::new(false));
  let shown = loud.clone();
  html! {
    section class={ SECTION } {
      h2 class={ HEADING } { "Children" }
      div class="space-y-2 text-sm" {
      p { { tags.iter().map(|&tag| Badge(tag)).collect::<Vec<_>>() } }
      p { "Numbers: " { 42 } ", " { 1.5 } " " { note } { nothing } }
      p { "Escaped: " code class="rounded bg-slate-100 px-1" { { "<b>not bold</b>" } } }
      p { "Raw HTML (trusted only): " #html{ || "<b>bold</b>" } }
      p {
        button class={ BUTTON } @click={ move |_| loud.update(|loud| *loud = !*loud) } { "Toggle" }
        " A closure child: "
        { move || if shown.value() { Some(html! { b class="text-emerald-600" { "now an element" } }) } else { None } }
      }
      }
    }
  }
}

pub async fn async_main() -> Result<(), JsValue> {
  let ctx = Context::new();
  ctx.provide_context(Rc::new(Store::default()));
  // Built inside `run`, every effect and listener belongs to `ctx`, so
  // `ctx.dispose()` would tear the whole page down.
  let app = ctx.run(|| {
    html! {
      main class="mx-auto max-w-2xl space-y-4 px-4 py-8" {
        h1 class="text-3xl font-bold tracking-tight" { "fluid" }
        { counter(&ctx) }
        { todos(&ctx) }
        { level_meter(&ctx) }
        { scopes(&ctx) }
        { children() }
      }
    }
  });
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
