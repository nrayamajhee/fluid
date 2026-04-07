# fluid

Like Solid but in Rust. 🤞

https://github.com/rustwasm/wasm-bindgen/issues/2853

Inspired by <https://youtu.be/J70HXl1KhWE?t=202>


```rust
let ctx = Context::new();
let counter = ctx.create_signal(0);
let p = html! {
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
    class=[ctx, [counter] -> if *counter.get() % 2 == 0  { "even" } else { "odd" }]
    {
      "Counter"
      ( " is: " )
      [ctx, [counter] -> counter.get().to_string()]
    }
    button
    @click=(move |_| {
      let new_val = *counter.get() + 1;
      counter.set(new_val);
    })
    { "+" }
  }
};
body().append_child(&p)?;
```
