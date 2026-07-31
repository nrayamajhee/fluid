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

### The Generated Code

The snippet above expands to the following:

```rust
{
    let el = gloo_utils::document().create_element("div")?;

    // style { r#"..."# }
    let child = {
        let el = gloo_utils::document().create_element("style")?;
        let child = {
            gloo_utils::document().create_text_node(
                "\n      .odd {\n          color: red;\n      }\n      .even {\n          color: blue;\n      }\n  ",
            )
        };
        el.append_child(&child)?;
        el
    };
    el.append_child(&child)?;

    // p id="test" class=#{ |counter| ... } { ... }
    let child = {
        let el = gloo_utils::document().create_element("p")?;

        // id="test"
        el.set_attribute("id", "test")?;

        // class=#{ |counter| ... }
        {
            let el = el.clone();
            let counter = counter.clone(); // one clone per listed signal
            ctx.create_effect(move || {
                let value = { if *counter.get() % 2 == 0 { "even" } else { "odd" } };
                el.set_attribute("class", value.as_ref())
                    .expect("Cannot setup attributes inside the effect");
            });
        }

        // "Counter"
        let child = { gloo_utils::document().create_text_node("Counter") };
        el.append_child(&child)?;

        // { " is: " }
        let child = { gloo_utils::document().create_text_node(" is: ") };
        el.append_child(&child)?;

        // #{ |counter| counter.get().to_string() }
        let child = {
            let node = gloo_utils::document().create_element("span")?;
            let n = node.clone();
            let counter = counter.clone();
            ctx.create_effect(move || {
                let inner_html = { counter.get().to_string() };
                n.set_inner_html(&inner_html);
            });
            node
        };
        el.append_child(&child)?;
        el
    };
    el.append_child(&child)?;

    // button @click={ ... } { "+" }
    let child = {
        let el = gloo_utils::document().create_element("button")?;
        let cl = Closure::wrap(Box::new(move |_| {
            let new_val = *counter.get() + 1;
            counter.set(new_val);
        }) as Box<dyn FnMut(web_sys::Event)>);
        el.add_event_listener_with_callback("click", cl.as_ref().unchecked_ref())?;
        cl.forget();

        let child = { gloo_utils::document().create_text_node("+") };
        el.append_child(&child)?;
        el
    };
    el.append_child(&child)?;

    el
}
```
