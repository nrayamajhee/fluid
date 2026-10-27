# fluid

Like Solid but in Rust. 🤞

https://github.com/rustwasm/wasm-bindgen/issues/2853

Inspired by <https://youtu.be/J70HXl1KhWE?t=202>

fluid is two crates: `fluid` (signals, effects, memos, owner scopes and the
DOM helpers) and `fluid-macro` (the `html!` macro). See [docs/studio-features.md](docs/studio-features.md)
for the roadmap and design.

[Example](#example) · [Signals](#signals-effects-and-memos) ·
[Scopes](#scopes-and-context-values) · [`html!`](#html) ·
[Coming from the original API](#coming-from-the-original-api) ·
[Testing](#testing)

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

## Example

[`fluid-example`](fluid-example/src/main.rs) is a one-page tour; each
section is one function. It is styled with Tailwind CSS, which
[Trunk](https://trunkrs.dev) builds from
[`src/tailwind.css`](fluid-example/src/tailwind.css) (Trunk downloads the
Tailwind CLI version pinned in `Trunk.toml`):

```sh
cargo install trunk
cd fluid-example && trunk serve
```

Tailwind finds classes by scanning the Rust sources, so write each class
name out whole. State-dependent classes are a good fit for toggles:
`class="flex gap-3" class:"text-slate-400 line-through"=#{ |done| done.value() }`.

| section      | shows                                                                                                                                                |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Counter**  | the original `class=#{ \|counter\| .. }` syntax, `#text` nodes, a memo, a typed `MouseEvent` handler                                                   |
| **Todos**    | keyed `for`, `if`/`else if`/`else`, a store from `use_context`, `batch`, memos, `prop:value`/`prop:checked`, `class:done`, a reactive `disabled`, an inline SVG icon |
| **Level**    | `match` with bindings and a guard, `@wheel.nonpassive.prevent`, `aria-*` attributes, an `Option` attribute (`title`), reactive SVG attributes, `set_if_changed`, a window listener from `fluid::listen(..).owned()` |
| **Scopes**   | a stopwatch inside an `if` branch: hiding it disposes its effects and runs its `on_cleanup` (stopping its timer); `untrack`                           |
| **Children** | an `IntoNode` component, `Vec` and `Option` children, numbers, escaped text, `#html` raw HTML, a `{ move \|\| .. }` closure child                        |

The whole page is built inside `ctx.run(..)`, so everything on it belongs to
one scope.

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
  @wheel.nonpassive.prevent={ move |e: WheelEvent| turn(e.delta_y()) }
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

## Coming from the original API

The original syntax and API still compile and behave the same (the few
edge cases are listed in [docs/studio-features.md](docs/studio-features.md#compatibility-summary)); these
are the newer ways to write common things.

| before                                                    | now                                                  |
| --------------------------------------------------------- | ---------------------------------------------------- |
| `let mut v = s.get().clone(); v.push(x); s.set(v);`       | `s.update(\|v\| v.push(x));`                          |
| `*s.get()`                                                | `s.value()` (or `s.with(\|v\| ..)` to borrow)          |
| `#{ \|s\| s.get().to_string() }` (a `<span>`, innerHTML)     | `#text{ \|s\| s.value() }` (a Text node, escaped)       |
| an empty element, then `append_child(&child)`             | `div { { child } }`                                  |
| `el.set_attribute("aria-label", ..)` after `html!`         | `aria-label=".."`                                    |
| `class=#{ \|on\| if on.value() { "a b on" } else { "a b" } }` | `class="a b" class:on=#{ \|on\| on.value() }`         |
| `add_event_and_forget` + `dyn_ref::<PointerEvent>()`      | `@pointerdown={ move \|e: PointerEvent\| .. }`         |
| a `hidden` class or HTML strings from an effect           | `if`, `match` and `for`                              |
| SVG through `set_inner_html`                              | `svg { path d=".." {} }`                             |
| several writes ordered to avoid a glitch                  | `fluid::batch(\|\| { .. })`                            |
| passing `Rc<Store>` through every function                | `ctx.provide_context(..)` / `ctx.use_context()`      |

Also kept: `fluid::add_event_and_forget`, `fluid::now()` and
`fluid::on_animation_frame(|delta| .., fps)`.

## Testing

fluid's only tests are component tests of the example
([`fluid-example/src/tests.rs`](fluid-example/src/tests.rs)): each section is
mounted on its own in headless Chrome and driven like a user would (typing,
clicking, scrolling, keys), then the DOM is checked.

```sh
cargo test -p fluid-example --target wasm32-unknown-unknown
```

They run through `wasm-bindgen-test-runner`
(`cargo install wasm-bindgen-cli --version <the locked wasm-bindgen>`),
which needs a `chromedriver` matching the installed Chrome
(`CHROMEDRIVER=...`).
