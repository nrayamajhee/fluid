# fluid

Like Solid but in Rust. 🤞

https://github.com/rustwasm/wasm-bindgen/issues/2853

Inspired by <https://youtu.be/J70HXl1KhWE?t=202>

fluid is two crates: `fluid` (signals, effects, memos, owner scopes and the
DOM helpers) and `fluid-macro` (the `html!` macro). `fluid-example` is a
counter and a todo list using most of what's below. See [PLAN.md](PLAN.md)
for the roadmap and design.

```rust
use fluid::Context;
use fluid_macro::html;

let ctx = Context::new();
let count = ctx.create_signal(0);
let page = html! {
  div {
    p { "Count: " #text{ |count| count.value() } }
    button @click={ move |_| count.update(|n| *n += 1) } { "+" }
  }
};
gloo_utils::body().append_child(&page)?;
```

## Signals, effects and memos

```rust
let ctx = Context::new();
let count = ctx.create_signal(0);         // Rc<Signal<i32>>

count.set(1);                             // notify subscribers
count.set_if_changed(1);                  // no-op when equal (T: PartialEq)
count.update(|n| *n += 1);                // mutate in place
let n = count.value();                    // copy out (T: Copy)
let v = count.cloned();                   // clone out (T: Clone)
let even = count.with(|n| n % 2 == 0);    // borrow briefly
let r = count.get();                      // Ref<T>; don't hold it across a write
let raw = count.get_untracked();          // read without subscribing

let doubled = ctx.create_memo({ let c = count.clone(); move || c.value() * 2 });
ctx.create_effect({ let d = doubled.clone(); move || log(d.value()) });

fluid::batch(|| { a.set(1); b.set(2); });  // effects run once, after both
fluid::untrack(|| count.value());          // read without subscribing
```

- Effects re-collect their dependencies on every run.
- Memos only notify when their value changes, and nothing ever sees a stale
  memo: an effect reading a signal and a memo of it runs once per write.
- A write inside an effect is processed after that effect returns.

## Scopes and context values

A `Context` is an owner scope. Effects, memos, listeners, cleanups and child
scopes created through it (or while it is the current scope) are disposed
together:

```rust
let ctx = Context::new();
let panel = ctx.child();
panel.create_effect(|| ..);
panel.on_cleanup(|| ..);
panel.dispose();                    // stops its effects, removes its listeners

ctx.provide_context(Rc::new(Store::default()));
let store = ctx.use_context::<Rc<Store>>();   // looked up the scope chain

// Everything html! creates inside `run` belongs to `ctx`:
let page = ctx.run(|| html! { main { .. } });
```

Dropping a `Context` never disposes it; only `dispose()` does. Effects
created while another effect runs belong to that effect and are disposed
before it re-runs. The free functions `fluid::create_effect`,
`create_memo`, `on_cleanup`, `provide_context` and `use_context` use the
current scope.

## `html!`

An element is its tag, its attributes and its children in braces:

```rust
html! { div class="card" { p { "Hello" } } }
```

`html!` returns a `web_sys::Element` (a `Text` for a root string or `{ expr }`).

### Children

| child                       | becomes                                                   |
| --------------------------- | --------------------------------------------------------- |
| `"text"`                    | a text node                                               |
| `{ expr }`                  | `expr` as a node (`Element`, `Node`, `Option`, `Vec`, anything `IntoNode`), or text from anything `Display` |
| `{ move \|\| expr }`         | the same, re-rendered when what it reads changes          |
| `#text{ \|a, b\| expr }`     | a text node updated in place when `a`/`b` change          |
| `#html{ \|a, b\| expr }`     | raw HTML in a `<span>` (only for trusted strings)         |
| `#{ \|a, b\| expr }`         | the original form: raw HTML in a `<span>`, owned by `ctx` |
| `tag attrs { .. }`          | an element                                                |
| `if`, `match`, `for`        | control flow, below                                       |

Components are functions returning a node: `{ knob(&ctx, chalk) }`.
Implement `fluid::IntoNode` for your own types.

The `|a, b|` list names the variables to clone into the reactive closure
(usually `Rc<Signal<_>>`s). Listing one that isn't used is fine.

### Attributes

```rust
input
  type="checkbox"                       // keywords
  aria-label="Name"                     // hyphenated
  data-id={ id }                        // any string, number, ...
  "x:name"="ok"                         // string-literal names
  disabled={ busy }                     // bool: present or absent
  title={ tooltip }                     // Option: None removes it
  class="base classes"
  class:"is-on ring-2"=#{ |on| on.value() }   // toggle whole class names
  prop:checked=#{ |on| on.value() }           // a DOM property
  value=#{ |v| v.cloned() }                   // reactive (legacy form, uses `ctx`)
  placeholder={ move || hint.cloned() }       // reactive, with your own captures
  {}
```

Attributes are set in source order; a duplicate is a compile error.
`class=#{..}` (a reactive whole class) can't be combined with `class:`
toggles.

### Events

```rust
button
  @click={ move |_| count.update(|n| *n += 1) }            // web_sys::Event
  @pointerdown={ move |e: PointerEvent| drag(e.client_x()) } // typed by annotation
  @wheel.nonpassive={ move |e: WheelEvent| { e.prevent_default(); turn(e.delta_y()) } }
  @mousedown.prevent={ |_| () }
  @click.once.capture={ |_| () }
  { "+" }
```

Modifiers: `capture`, `once`, `passive`, `nonpassive`, `prevent` (calls
`preventDefault` first) and `stop` (`stopPropagation`). Listeners are removed
when their scope is disposed. For listeners on `window` or `document`, use
`fluid::listen(&target, "keydown", ListenerOptions::new(), |e: KeyboardEvent|
..)`; it returns a `Listener` that is removed on drop unless you call
`.owned()` (tie it to the current scope) or `.forget()`.

### SVG

`svg` and everything inside it (up to `foreignObject`) is created in the SVG
namespace, as is an `html!` whose root is an SVG-only tag (`path`, `g`,
`circle`, ...).

### Control flow

```rust
div {
  if |open| open.value() {
    div class="panel" { .. }
  } else if count.value() > 10 {
    "big"
  } else {
    "closed"
  }

  match |mode| mode.value() {
    Mode::Idle => { "idle" }
    Mode::Playing(n) => { "playing " { n } }
  }

  ul {
    for |todos| todo in todos.cloned(), key = todo.id {
      li { { todo.title.clone() } }
    }
  }
}
```

- `if`/`match` only rebuild when the chosen branch changes (or, for a `match`
  arm that binds values, when those change). Each branch lives in its own
  scope and is disposed when swapped out.
- `for` is keyed: rows are built once per key, moved when the order changes
  and disposed when their key goes. A row keeps the item it was built with;
  put changing per-row data in signals inside the item.
- The capture list on the first `if`/`match`/`for` covers the conditions
  and every branch or row. Names listed in nested `#text{ |..| }` etc. are
  cloned automatically; other non-`Copy` locals a branch uses must be listed.
- Inside a branch or row, `ctx` refers to that branch's scope.

## Testing

```sh
cargo test                      # reactive core, keyed-list logic, macro (trybuild)
cargo test -p fluid --target wasm32-unknown-unknown   # DOM tests, headless
```

The DOM tests run in headless Chrome through `wasm-bindgen-test-runner`
(`cargo install wasm-bindgen-cli --version <the locked wasm-bindgen>`), which
needs a `chromedriver` matching the installed Chrome (`CHROMEDRIVER=...`).

`compat/` holds every `html!` block of
[studio's web-rust UI](https://github.com/nrayamajhee/studio) and
[wgpu-test's pause menu](https://github.com/nrayamajhee/wgpu-test) verbatim,
so `cargo build --workspace --target wasm32-unknown-unknown` checks that
fluid still compiles them. Regenerate them with
`python3 compat/scripts/extract.py <studio> <wgpu-test>`.
