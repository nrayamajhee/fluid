# fluid roadmap

> Status: phases (a)–(e) are implemented on `claude/fluid-roadmap`; (f) is
> future work. See the PR for the checklist.

fluid was used to port the "Device" synth UI of `nrayamajhee/studio` to a Trunk
wasm app (`web-rust/`). That port, together with `nrayamajhee/wgpu-test`
(`src/ui/pause_menu.rs`), is the real-world reference for this plan. Every item
below is **additive**: today's syntax and API keep compiling and behaving the
same. The few places where that is not strictly true are called out under
_Compatibility_.

Contents: [Problems and proposed API](#problems-and-proposed-api) ·
[Design](#design) · [Phases](#phases) · [Compatibility](#compatibility-summary)

---

## Problems and proposed API

### 1. Element/Node children

Today `{ expr }` always becomes `create_text_node(expr)` and must be a `&str`,
so parents are built as empty shells and filled with `append_child`:

```rust
// studio web-rust/src/ui/mod.rs
let main = html! { main class="absolute inset-0 z-10 py-20" {} };
append(&main, [synth(ctx, device)?])?;
append(&page, [sky::sky()?, main, footer()?, corner::corner(ctx, device)?])?;
```

Proposed: `{ expr }` inserts whatever `expr` is.

| value                                                    | inserted as                    |
| -------------------------------------------------------- | ------------------------------ |
| implements `fluid::IntoNode` (Option, Vec, user types…)  | its node(s)                    |
| any `AsRef<web_sys::Node>` (Element, HtmlElement, Text…) | the node itself                |
| any `Display` (`&str`, `String`, numbers…)               | a Text node (escaped)          |
| a closure literal `move \|\| expr`                       | a reactive child (see 9)       |

```rust
html! {
  div class="relative h-dvh w-full" {
    { sky::sky()? }
    main class="absolute inset-0 z-10 py-20" { { synth(ctx, device)? } }
    { footer()? }
    { count.value() }          // Display → text
  }
}
```

Dispatch is resolved at the macro call site by autoref specialization over
the concrete type, so no blanket-impl coherence problems and `&str`/`&String`
(the only things that compiled before) still produce the same Text node.
A component is just a function returning a node, so `{ knob(ctx, chalk)? }`
is the component-call form; no extra syntax is needed.

### 2. Events

```rust
// studio web-rust/src/ui/knob.rs — the macro can't type the event or set options
add_event_and_forget(&element, "wheel", move |event| {
  let Some(event) = event.dyn_ref::<WheelEvent>() else { return };
  event.prevent_default();
  ...
});
```

Proposed:

```rust
button
  @pointerdown={ move |e: PointerEvent| start(e.pointer_id()) }  // typed by annotation
  @wheel.nonpassive={ move |e: WheelEvent| { e.prevent_default(); turn(e.delta_y()) } }
  @mousedown.prevent={ |_| () }        // .prevent / .stop call preventDefault / stopPropagation first
  @click.once.capture={ |_| () }       // listener options
  { "+" }
```

- The event type comes from an explicit type on the closure's first parameter.
  Unannotated closures (and non-closure handlers) keep getting
  `web_sys::Event`. A name→type map (`click` → `MouseEvent`) was considered
  and rejected for `@`: it would change the type of today's `|e|` and break
  code that passes `e` on as `Event`.
- Modifiers: `capture`, `once`, `passive`, `nonpassive`, `prevent`, `stop`.
- Listeners are no longer always `forget()`-ed: when an owner scope is active
  (inside a control-flow branch, `ctx.run`, an effect), the listener is
  removed and its closure freed when that scope is disposed. With no active
  owner it is forgotten exactly as today.
- New `fluid::listen(&target, "wheel", ListenerOptions::nonpassive(), |e:
  WheelEvent| ..) -> Listener` for listeners outside `html!` (window,
  document); `Listener::remove()` / `Listener::forget()`.
  `add_event_and_forget` stays.

### 3. Attributes

```rust
// studio: hyphenated names and keywords are impossible inside html!
let body = html! { div role="group" class="..." {} };
body.set_attribute("aria-label", "Synthesizer")?;
```

Proposed:

```rust
input
  type="checkbox"                    // keywords
  aria-label="Synthesizer"           // hyphenated
  data-step={ step }                 // any Display/str value
  "x-weird:name"="ok"                // string-literal names
  disabled={ busy }                  // bool → present/absent
  title={ maybe_title }              // Option → None removes
  prop:checked=#{ |on| on.value() }  // DOM property, not attribute
  class="pad base classes"
  class:"bg-pad-held shadow-pad-held"=#{ |held| held.value() } // class toggle
  {}
```

- Names: identifiers (keywords allowed), `-`-joined identifiers, string
  literals, and `ns:name` (e.g. `xlink:href`). `class:` and `prop:` are the
  two reserved prefixes.
- Values go through `fluid::AttrValue`: `&str`/`String`/anything `AsRef<str>`
  set the attribute, `bool` adds/removes it, `Option<T>` removes on `None`,
  numbers are formatted. Reactive values use the same trait, so
  `disabled=#{ |busy| busy.value() }` works.
- Attributes are kept in source order (a `Vec`), and a duplicate name is a
  compile error instead of a silent overwrite.
- Class toggles accept several whitespace-separated classes in one string so
  Tailwind sees whole class names in source; they combine with a static
  `class="..."`. Mixing a reactive `class=#{}` with `class:` toggles is a
  compile error (the reactive string would clobber the toggles).

### 4. SVG

```rust
// studio web-rust/src/ui/screen.rs
const SCOPE: &str = r#"<svg width="100%" ...><line .../></svg>"#;
scope.set_inner_html(SCOPE);
```

Proposed: `svg` and every element below it (until `foreignObject`) is created
with `createElementNS("http://www.w3.org/2000/svg", ..)`. An `html!` whose
root is an SVG-only tag (`path`, `g`, `circle`, `line`, …) is created in the
SVG namespace too, so SVG fragments can be split into functions.

```rust
html! {
  svg width="100%" height="100%" aria-hidden="true" {
    line x1="0" y1="50%" x2="100%" y2="50%" stroke="currentColor" stroke-dasharray="3 5" {}
  }
}
```

### 5. Control flow

```rust
// studio: a hidden class instead of a conditional node, HTML strings from effects
div class=#{ |overlay| if overlay.get().is_some() { OVERLAY } else { "hidden" } } { ... }
#{ |values, preset| if unsaved(*preset.get(), &values.get()) { unsaved_mark() } else { String::new() } }
```

Proposed child nodes:

```rust
if |overlay| overlay.with(Option::is_some) {
  div class={ OVERLAY } { ... }
} else if |x| ... {        // only the first `if` takes a capture list; it covers every branch
} else {
  "nothing"
}

match |state| state.value() {
  Mode::Idle => { "idle" }
  Mode::Playing(n) => { span { "playing " { n } } }
}

ul {
  for |todos| todo in todos.cloned(), key = todo.id {
    li { { todo.title.clone() } }
  }
}
```

- `if`/`match` re-evaluate their condition in an effect and only rebuild when
  the chosen branch changes (for `match`, also when the arm binds values).
  Each branch is built in a child owner scope that is disposed (effects
  stopped, listeners removed) when the branch is swapped out.
- `for … in …, key = …` is a keyed list: on change it builds rows for new
  keys, disposes rows for removed keys and moves existing rows into the new
  order without rebuilding them. Rows see the item value they were created
  with; per-row live data goes in signals inside the item (as Solid's `For`).
- Every dynamic region is delimited by an empty comment anchor, so it works
  anywhere in a parent, including next to static siblings.

### 6. Signal API

```rust
// studio device.rs — every update clones the whole value
let mut held = self.held.get().clone();
held.insert(root, notes);
self.held.set(held);
// and this panics with BorrowMutError:
let v = signal.get(); signal.set(*v + 1);
```

Proposed additions to `Signal<T>` (all existing methods unchanged):

```rust
held.update(|held| { held.insert(root, notes); });   // mutate in place, notify once
let empty = held.with(|held| held.is_empty());        // borrow without a Ref escaping
let n = count.value();                                // T: Copy
let v = values.cloned();                              // T: Clone
octave.set_if_changed(next);                          // T: PartialEq; no notify when equal
let raw = count.get_untracked();                      // read without subscribing
fluid::batch(|| { selected.set(s); preset.set(p); }); // effects run once, after both writes
fluid::untrack(|| ..);                                // reads inside don't subscribe
```

`batch` fixes the `select_preset` glitch: writes inside a batch only mark
effects; they run once, after the batch, seeing all new values.

### 7. Derived values

```rust
let param = ctx.create_memo({ let d = device.clone(); move || d.param().0.name });
// a Memo<T> is a cheap Clone handle with get/with/value/cloned like Signal
```

Memos only notify when their value changes (`T: PartialEq`) and are
glitch-free: an effect that reads both a signal and a memo of it runs once
per write and never sees a stale memo.

### 8. Lifetimes, owners, context values

Today an effect is subscribed to every signal it ever read, forever, and
nothing can be disposed. Proposed:

```rust
let ctx = Context::new();          // a root owner (kept alive until disposed)
let child = ctx.child();           // nested scope
child.create_effect(..);           // owned by `child`
child.on_cleanup(|| ..);
child.dispose();                   // stops its effects, removes listeners, runs cleanups

ctx.provide_context(store.clone());            // Rc<Store>, no more prop drilling
let store = ctx.use_context::<Store>().unwrap(); // looks up the owner chain
```

- Effects re-collect their dependencies on every run and unsubscribe from
  ones they no longer read.
- Effects (and the owner scopes built by control flow) created while another
  effect runs belong to it and are disposed before it re-runs.
- **Dropping a `Context` does not dispose it** — studio and wgpu-test create
  `Context::new()` in a function and drop it at the end, relying on effects
  living on. Disposal is always explicit.

### 9. Reactive text

```rust
// #{ } wraps in a <span> and uses innerHTML: extra DOM, and XSS with user strings
span { #{ |octave| octave_label(*octave.get()) } }
```

Proposed:

```rust
span { #text{ |octave| octave_label(octave.value()) } } // a Text node, updated via .data
span { { move || count.value() } }                      // same, with your own captures
span { #html{ |preset| icon_svg(preset.value()) } }      // explicit raw-HTML opt-in
```

`#{ |..| }` keeps its exact current behaviour (span + innerHTML) because
studio relies on it for SVG icons and `&nbsp;`; changing it would compile but
render escaped HTML. It is documented as legacy, equivalent to `#html{}`.

### 10. Macro hygiene and ergonomics

- Generated code uses fully qualified `::fluid::…` paths, so callers no longer
  need `gloo_utils`, `Closure` or `JsCast` in scope.
- A listed capture that isn't used no longer warns.
- `ctx` is only needed by the legacy `#{ }` / `name=#{ }` forms (as today);
  everything new uses the active owner and works without a `ctx` variable.
- Tag and attribute names are validated at compile time, so element creation
  can't fail at run time; generated code stops using `?` (see Compatibility)
  and effect attributes stop `.expect(...)`-ing.
- Remove the unused `maud` dependency from `fluid-macro`.
- Auto-cloning captured names was considered and rejected: a proc macro can't
  tell locals from consts, statics, functions or bindings made inside the
  expression, so any heuristic either misses captures or emits clones of
  names that don't exist. The capture list stays, and `#text`, `if`, `match`
  and `for` all use the same `|a, b|` list. Copy-able signal handles (see
  phase f) are the real fix.

---

## Design

### Reactive core (`fluid/src/reactive.rs`, DOM-free)

- `Signal<T>` = `RefCell<T>` + an `Rc<Source>`. A `Source` holds weak links to
  its observer nodes. `create_signal` still returns `Rc<Signal<T>>`.
- A **node** is an effect or a memo computation: its closure, the sources it
  read on the last run, a state (`Clean`/`Check`/`Dirty`) and its own owner.
- Writes use push-pull colouring (as Reactively / Preact signals): a write
  marks direct observers `Dirty` and everything downstream of a memo `Check`,
  queueing effects. Flushing runs each queued effect only if it is `Dirty`,
  or `Check` and one of its memo sources actually changed when pulled.
  Reading a memo always pulls it up to date first, so nothing sees a stale
  value (glitch-free) and an effect runs at most once per flush.
- Each run first unsubscribes the node from all old sources and disposes its
  child owner's contents, then re-tracks.
- `batch` increments a depth counter; flushing happens when it returns to
  zero. Every node run is implicitly batched, so an effect that writes a
  signal never re-enters another effect; the write is processed after it
  returns. Outside effects/batches, `set` still flushes synchronously, as
  today. A flush that runs more than 100 000 nodes panics with a "cycle?"
  message instead of hanging the tab.

### Owners

- `Owner` tree: children, owned nodes, cleanups, context values, parent.
- `Context` is a cloneable handle to an owner. `Context::new()` registers a
  root in a thread-local list so it lives until `dispose()`.
- A thread-local _current owner_ is set while a node runs, inside
  `ctx.run(..)`, and while a control-flow branch is built.
- `ctx.create_effect` attaches to the current owner when it is inside `ctx`,
  or when `ctx` is a root (apps pass their one root `ctx` everywhere, so it
  stands for "wherever this runs"); otherwise to `ctx`. That keeps explicit
  child scopes explicit, while effects created through a captured root `ctx`
  during an effect's re-run (legacy `#{}` inside a reactive child) are still
  disposed before the next run instead of piling up.
- To make a whole tree disposable, build it inside `ctx.run(|| html! {..})`:
  every form, legacy or new, then belongs to `ctx`.
- Free functions (`fluid::create_effect`, `on_cleanup`, `use_context`, …) use
  the current owner, falling back to a detached root (never disposed — today's
  behaviour).

### Macro grammar (all additions)

```
node      := LitStr                                  // text (unchanged)
           | '{' expr '}'                            // static child, now IntoNode/Node/Display; closure literal → reactive
           | '#' '{' caps expr '}'                   // legacy: span + innerHTML (unchanged)
           | '#html' '{' caps expr '}'               // explicit raw HTML (same as legacy)
           | '#text' '{' caps expr '}'               // reactive Text node
           | 'if' caps? cond '{' node* '}' ('else' 'if' cond '{' node* '}')* ('else' '{' node* '}')?
           | 'match' caps? expr '{' (pat ('if' guard)? '=>' '{' node* '}' ','?)* '}'
           | 'for' caps? pat 'in' expr ',' 'key' '=' expr '{' node* '}'
           | tag attr* '{' node* '}'
caps      := '|' ident (',' ident)* ','? '|'
attr      := '@' ident ('.' modifier)* '=' '{' expr '}'
           | name '=' (LitStr | '{' expr '}' | '#' '{' caps expr '}')
name      := ident ('-' ident)* | LitStr | ident ':' (ident ('-' ident)* | LitStr)
```

`if`, `match`, `for` are Rust keywords and so could never be tag names; `#`
followed by an identifier was a parse error. None of this changes the meaning
of anything that parsed before.

---

## Phases

Each phase is one or more commits on `claude/fluid-roadmap`.

### (a) Signal API, batch, untrack, memos

Signal methods from §6, `batch`, `untrack`, `create_memo`, new reactive core
(colouring, queue, re-tracking).
_Risk_: changing effect scheduling (writes inside an effect are now deferred
until it returns instead of re-entering).

### (b) Owner scopes, disposal, context values

`Context` as owner handle, `child`, `dispose`, `on_cleanup`, `run`,
`provide_context`/`use_context`, nested-effect disposal on re-run.
_Risk_: lifetime regressions (things disposed too early). Mitigated by never
disposing on drop.

### (c) `html!` grammar

Node children, Display text, `#text`/`#html`, attribute names, ordered attrs
with duplicate errors, `AttrValue`, `prop:`, class toggles, SVG namespace,
typed events + modifiers + owner-tied listeners.
_Risk_: dispatch changes for `{ expr }`/attribute values; covered by keeping
every previously-accepted type producing identical calls, and by building
studio and wgpu-test against the branch with `[patch]` before merging.

### (d) Control flow

`if`/`else if`/`else`, `match`, keyed `for`, region anchors, branch scopes.
_Risk_: DOM region bookkeeping bugs (wrong order after moves).

### (e) Hygiene and cleanup

Qualified paths, no `?`/`.expect`, unused-capture warnings, drop `maud`,
README + example updates, clippy/fmt clean.

### Tests

fluid is tested only through its example: `fluid-example/src/tests.rs`
mounts the todo app in headless Chromium and drives it as a user would
(adding, completing, filtering, clearing, removing), checking the DOM. That
covers signals, the memo and `batch`; context values and component scopes;
the `html!` attribute, event and child forms; and `if`, `match` and keyed
`for`, including that a keyed row keeps its DOM node.

### (f) Later / not in this PR

- Copy signal handles (arena-backed `ReadSignal`/`WriteSignal`) so closures
  don't need clones at all.
- Keyed-list moves via a longest-increasing-subsequence pass (today's
  backward walk is correct but can move more rows than needed, e.g. a
  rotation).
- Non-keyed `for` over indices, `Show`/`Suspense`-style async helpers.
- Server-side rendering.

---

## Compatibility summary

Nothing that compiles today should stop compiling, with these caveats:

1. **Effect scheduling**: a `set` _inside an effect_ used to run subscribers
   immediately, nested inside the running effect; it now runs them right
   after the effect returns. `set` from event handlers and plain code is
   still synchronous. No known user depends on the nesting.
2. **No `?` in generated code**: `html!` no longer requires a
   `Result<_, JsValue>` context. The only way this breaks is a closure whose
   error type was inferred _solely_ from the macro's `?`; migration: annotate
   it (`Ok::<_, JsValue>(el)`).
3. **Generated paths**: generated code refers to `::fluid::…`, so a crate using
   `html!` must depend on `fluid` (both known users do). Imports that existed
   only for the macro (`use wasm_bindgen::JsCast;`) may become unused-import
   warnings.
4. **Effect signature**: `create_effect` takes `impl FnMut()` instead of
   `impl Fn()` — strictly more permissive.
5. **Duplicate attributes** (`div class="a" class="b"`) are now a compile
   error instead of a silent, order-dependent overwrite. Migration: delete
   the one you didn't mean.
6. **Attribute values** are matched by trait instead of `&str` deref
   coercion. Everything that coerced to `&str` within two derefs still works
   (`&String`, `&Rc<String>`, `&signal.get()`); a deeper chain needs an
   explicit `.as_str()`.

Before merging, build both downstream apps against this branch with a
`[patch]` on the fluid git URL; they pick it up on their next `cargo update`
because their dependency isn't pinned to a commit.
