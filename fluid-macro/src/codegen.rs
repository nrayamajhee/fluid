use proc_macro2::TokenStream as TS;
use quote::{quote, quote_spanned};

use crate::ast::{Attr, AttrName, AttrValue, Element, Node, Reactive};

/// Elements that only exist in SVG, created in its namespace even at the
/// root of an `html!` (outside an `svg`).
const SVG_ONLY: &[&str] = &[
  "animate",
  "animateMotion",
  "animateTransform",
  "circle",
  "clipPath",
  "defs",
  "desc",
  "ellipse",
  "feBlend",
  "feColorMatrix",
  "feGaussianBlur",
  "feOffset",
  "filter",
  "foreignObject",
  "g",
  "line",
  "linearGradient",
  "marker",
  "mask",
  "path",
  "pattern",
  "polygon",
  "polyline",
  "radialGradient",
  "rect",
  "stop",
  "symbol",
  "text",
  "textPath",
  "tspan",
  "use",
];

fn private() -> TS {
  quote! { ::fluid::__private }
}

/// The root of an `html!`. Keeps today's types: an element gives
/// `Element`, a literal or `{ expr }` a `Text`.
pub fn build_root(node: Node) -> TS {
  let p = private();
  match node {
    Node::Expr(expr) => quote! { #p::text(#expr) },
    Node::Element(element) => {
      let svg = SVG_ONLY.contains(&element.name.as_str());
      build_element(element, svg)
    }
    node => build_child(node, false),
  }
}

/// Clones each capture for a `move` closure; unused ones don't warn.
fn clone_captures(captures: &[syn::Ident]) -> TS {
  quote! {
    #(
      #[allow(unused_variables)]
      let #captures = ::std::clone::Clone::clone(&#captures);
    )*
  }
}

/// Converts `value` to a `Child` by autoref specialization.
fn to_child(value: TS) -> TS {
  let p = private();
  quote! {{
    #[allow(unused_imports)]
    use #p::{ViaAsNode as _, ViaDisplay as _, ViaIntoNode as _};
    (&&&#p::Wrap::new(#value)).__child()
  }}
}

/// An expression for a child node (anything `AsRef<web_sys::Node>`).
fn build_child(node: Node, svg: bool) -> TS {
  let p = private();
  match node {
    Node::Text(text) => quote! { #p::text(#text) },
    Node::Expr(expr) => {
      let child = to_child(expr);
      quote! { #child.into_node() }
    }
    Node::Closure(closure) => {
      let child = to_child(quote! { __fluid_value });
      quote! {{
        #[allow(unused_mut)]
        let mut __fluid_render = #closure;
        #p::dynamic(move || {
          let __fluid_value = __fluid_render();
          #child
        })
      }}
    }
    Node::ReactiveText(Reactive { captures, expr }) => {
      let clones = clone_captures(&captures);
      let child = to_child(quote! { __fluid_value });
      quote! {{
        #clones
        #p::dynamic(move || {
          let __fluid_value = { #expr };
          #child
        })
      }}
    }
    // `#{ }` is unchanged: raw HTML in a span, owned by `ctx`.
    Node::Html {
      reactive: Reactive { captures, expr },
      legacy,
    } => {
      let clones = clone_captures(&captures);
      let create_effect = if legacy {
        quote! { ctx.create_effect }
      } else {
        quote! { ::fluid::create_effect }
      };
      quote! {{
        let __fluid_node = #p::element("span");
        let __fluid_n = __fluid_node.clone();
        #clones
        #create_effect(move || {
          let inner_html = { #expr };
          __fluid_n.set_inner_html(&inner_html);
        });
        __fluid_node
      }}
    }
    Node::Element(element) => build_element(element, svg),
  }
}

fn build_element(element: Element, svg: bool) -> TS {
  let p = private();
  let Element {
    name,
    span,
    attrs,
    children,
  } = element;
  let svg = svg || name == "svg";
  let create = if svg {
    quote_spanned! {span=> #p::svg_element(#name) }
  } else {
    quote_spanned! {span=> #p::element(#name) }
  };
  let attrs: Vec<TS> = attrs.into_iter().map(build_attr).collect();
  let child_svg = svg && name != "foreignObject";
  let children: Vec<TS> = children
    .into_iter()
    .map(|child| build_child(child, child_svg))
    .collect();
  quote! {{
    let __fluid_el: #p::web_sys::Element = #create;
    #(#attrs)*
    #(
      let __fluid_child = #children;
      #p::append(&__fluid_el, &__fluid_child);
    )*
    __fluid_el
  }}
}

/// Applies `value` (an expression) to the element for an attribute name.
fn apply(name: &AttrName, value: TS) -> TS {
  let p = private();
  match name {
    AttrName::Plain(name) => quote! { ::fluid::AttrValue::apply(#value, &__fluid_el, #name); },
    AttrName::Class(classes) => quote! { #p::toggle_class(&__fluid_el, #classes, #value); },
    AttrName::Prop(name) => quote! { #p::set_property(&__fluid_el, #name, #value); },
  }
}

fn build_attr(attr: Attr) -> TS {
  let p = private();
  match attr {
    Attr::Attr { name, value, .. } => match value {
      AttrValue::Lit(lit) => match &name {
        AttrName::Plain(name) => quote! { #p::set_attribute(&__fluid_el, #name, #lit); },
        _ => apply(&name, quote! { #lit }),
      },
      AttrValue::Expr(expr) => apply(&name, expr),
      AttrValue::Closure(closure) => {
        let set = apply(&name, quote! { __fluid_render() });
        quote! {{
          #[allow(unused_mut)]
        let mut __fluid_render = #closure;
          let __fluid_el = __fluid_el.clone();
          ::fluid::create_effect(move || { #set });
        }}
      }
      // The original reactive attribute, owned by `ctx`.
      AttrValue::Reactive(Reactive { captures, expr }) if matches!(name, AttrName::Plain(_)) => {
        let clones = clone_captures(&captures);
        let set = apply(&name, quote! { value });
        quote! {{
          let __fluid_el = __fluid_el.clone();
          #clones
          ctx.create_effect(move || {
            let value = { #expr };
            #set
          });
        }}
      }
      AttrValue::Reactive(Reactive { captures, expr }) => {
        let clones = clone_captures(&captures);
        let set = apply(&name, quote! { value });
        quote! {{
          let __fluid_el = __fluid_el.clone();
          #clones
          ::fluid::create_effect(move || {
            let value = { #expr };
            #set
          });
        }}
      }
    },
    Attr::Event {
      name,
      modifiers,
      handler,
      event_type,
    } => {
      let has = |wanted: &str| modifiers.iter().any(|modifier| modifier == wanted);
      let (capture, once, prevent, stop) =
        (has("capture"), has("once"), has("prevent"), has("stop"));
      let passive = if has("passive") {
        quote! { ::std::option::Option::Some(true) }
      } else if has("nonpassive") {
        quote! { ::std::option::Option::Some(false) }
      } else {
        quote! { ::std::option::Option::None }
      };
      let event_type = match event_type {
        Some(ty) => quote! { #ty },
        None => quote! { #p::web_sys::Event },
      };
      quote! {
        #p::on::<#event_type>(
          &__fluid_el,
          #name,
          ::fluid::ListenerOptions { capture: #capture, once: #once, passive: #passive },
          #prevent,
          #stop,
          #handler,
        );
      }
    }
  }
}
