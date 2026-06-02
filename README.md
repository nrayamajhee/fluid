# fluid

Like Solid but in Rust. 🤞

https://github.com/rustwasm/wasm-bindgen/issues/2853

Inspired by <https://youtu.be/J70HXl1KhWE?t=202>

The `html!` macro allows you to build HTML element structures inside Rust. 

* **Elements:** Created using the tag name followed by attributes and braces `{ ... }` for children.
  ```rust
  div { p { "Hello World" } }
  ```
* **Static Expressions:** Any standard curly-braced block `{ ... }` acts as a static expression, completely supporting standard Rust closures or arbitrary code blocks.
  ```rust
  p { { " is: " } }
  ```
* **Reactive Effects:** Prefixed with `#`, the `#{ |signals| expression }` syntax specifies reactive signals to clone and track.
  ```rust
  p class=#{ |counter| if *counter.get() % 2 == 0 { "even" } else { "odd" } } {
    #{ |counter| counter.get().to_string() }
  }
  ```
* **Event Handlers:** Declared with `@event_name={ callback_closure }`.
  ```rust
  button @click={ move |_| { counter.set(*counter.get() + 1); } } { "+" }
  ```

### The `html!` Macro Snippet

```rust
html! {
  div {
    style { r#"
          .odd {
              color: red;
          }
          .even {
              color: blue;
          }
      "#}
    p
    id="test"
    class=#{ |counter| if *counter.get() % 2 == 0 { "even" } else { "odd" } }
    {
      "Counter"
      { " is: " }
      #{ |counter| counter.get().to_string() }
    }
    button
    @click={ move |_| {
      let new_val = *counter.get() + 1;
      counter.set(new_val);
    }}
    { "+" }
  }
}
```
