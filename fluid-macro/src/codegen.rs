use proc_macro2::TokenStream as TS;
use quote::{quote, quote_spanned};

use syn::{Ident, Pat};

use crate::ast::{Arm, Attr, AttrName, AttrValue, Element, Node, Reactive};

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
      // A bare expression keeps its temporaries alive for the whole call, as
      // before; statements (`{ a; b }`) need their own block.
      let expr = if syn::parse2::<syn::Expr>(expr.clone()).is_ok() {
        expr
      } else {
        quote! {{ #expr }}
      };
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
    Node::If {
      captures,
      branches,
      otherwise,
    } => {
      let mut names = captures;
      for (_, children) in &branches {
        names.extend(nested_captures(children));
      }
      if let Some(children) = &otherwise {
        names.extend(nested_captures(children));
      }
      let names = dedup(names);
      let clones = clone_captures(&names);
      let count = branches.len();
      let arms = branches
        .into_iter()
        .enumerate()
        .map(|(index, (cond, children))| {
          let branch = build_branch(children, &names, svg);
          quote! { if #cond { (#index, false, #branch) } }
        });
      let otherwise = build_branch(otherwise.unwrap_or_default(), &names, svg);
      quote! {{
        #clones
        #p::switch(move || #(#arms else)* { (#count, false, #otherwise) })
      }}
    }
    Node::Match {
      captures,
      expr,
      arms,
    } => {
      let mut names = captures;
      for arm in &arms {
        let bound = bindings(&arm.pat);
        names.extend(
          nested_captures(&arm.children)
            .into_iter()
            .filter(|name| !bound.contains(name)),
        );
      }
      let names = dedup(names);
      let clones = clone_captures(&names);
      let arms = arms.into_iter().enumerate().map(
        |(
          index,
          Arm {
            pat,
            guard,
            children,
          },
        )| {
          let binds = !bindings(&pat).is_empty();
          let guard = guard.map(|guard| quote! { if #guard });
          let branch = build_branch(children, &names, svg);
          quote! { #pat #guard => (#index, #binds, #branch), }
        },
      );
      quote! {{
        #clones
        #p::switch(move || match #expr { #(#arms)* })
      }}
    }
    Node::For {
      captures,
      pat,
      items,
      key,
      children,
    } => {
      let bound = bindings(&pat);
      let mut names = captures;
      names.extend(
        nested_captures(&children)
          .into_iter()
          .filter(|name| !bound.contains(name)),
      );
      let names = dedup(names);
      let clones = clone_captures(&names);
      let row = build_nodes(children, svg);
      quote! {{
        let __fluid_items = {
          #clones
          move || ::std::iter::IntoIterator::into_iter(#items).collect::<::std::vec::Vec<_>>()
        };
        #p::each(
          __fluid_items,
          move |__fluid_item: &_| {
            #[allow(unused_variables)]
            let #pat = __fluid_item;
            #key
          },
          {
            #clones
            move |__fluid_item| {
              #[allow(unused_variables)]
              let #pat = __fluid_item;
              #clones
              #[allow(unused_variables)]
              let ctx = &::fluid::Context::current();
              #row
            }
          },
        )
      }}
    }
  }
}

/// Several nodes as one `web_sys::Node` (a fragment).
fn build_nodes(children: Vec<Node>, svg: bool) -> TS {
  let p = private();
  let children = children.into_iter().map(|child| build_child(child, svg));
  quote! {{
    let __fluid_frag = #p::fragment();
    #(
      let __fluid_child = #children;
      #p::append(&__fluid_frag, &__fluid_child);
    )*
    ::std::convert::Into::<#p::web_sys::Node>::into(__fluid_frag)
  }}
}

/// A boxed builder for a branch, owning fresh clones of the captures and
/// binding `ctx` to the branch's scope.
fn build_branch(children: Vec<Node>, names: &[Ident], svg: bool) -> TS {
  let p = private();
  let clones = clone_captures(names);
  let nodes = build_nodes(children, svg);
  quote! {{
    #clones
    ::std::boxed::Box::new(move || {
      #[allow(unused_variables)]
      let ctx = &::fluid::Context::current();
      #nodes
    }) as #p::Branch
  }}
}

/// Capture names listed anywhere below, minus names bound by inner
/// patterns: an enclosing branch must clone them too.
fn nested_captures(nodes: &[Node]) -> Vec<Ident> {
  let mut names = Vec::new();
  for node in nodes {
    match node {
      Node::Html {
        reactive: Reactive { captures, .. },
        ..
      }
      | Node::ReactiveText(Reactive { captures, .. }) => names.extend(captures.iter().cloned()),
      Node::Element(element) => {
        for attr in &element.attrs {
          if let Attr::Attr {
            value: AttrValue::Reactive(Reactive { captures, .. }),
            ..
          } = attr
          {
            names.extend(captures.iter().cloned());
          }
        }
        names.extend(nested_captures(&element.children));
      }
      Node::If {
        captures,
        branches,
        otherwise,
      } => {
        names.extend(captures.iter().cloned());
        for (_, children) in branches {
          names.extend(nested_captures(children));
        }
        if let Some(children) = otherwise {
          names.extend(nested_captures(children));
        }
      }
      Node::Match { captures, arms, .. } => {
        names.extend(captures.iter().cloned());
        for arm in arms {
          let bound = bindings(&arm.pat);
          names.extend(
            nested_captures(&arm.children)
              .into_iter()
              .filter(|name| !bound.contains(name)),
          );
        }
      }
      Node::For {
        captures,
        pat,
        children,
        ..
      } => {
        names.extend(captures.iter().cloned());
        let bound = bindings(pat);
        names.extend(
          nested_captures(children)
            .into_iter()
            .filter(|name| !bound.contains(name)),
        );
      }
      Node::Text(_) | Node::Expr(_) | Node::Closure(_) => {}
    }
  }
  names
}

fn dedup(names: Vec<Ident>) -> Vec<Ident> {
  let mut unique: Vec<Ident> = Vec::new();
  for name in names {
    if !unique.contains(&name) {
      unique.push(name);
    }
  }
  unique
}

/// Names a pattern binds. A capitalised bare name (`None`, an imported
/// variant) is taken to be a constant, not a binding.
fn bindings(pat: &Pat) -> Vec<Ident> {
  let mut names = Vec::new();
  collect_bindings(pat, &mut names);
  names
}

fn collect_bindings(pat: &Pat, names: &mut Vec<Ident>) {
  match pat {
    Pat::Ident(ident) => {
      let constant = ident.subpat.is_none()
        && ident.by_ref.is_none()
        && ident.mutability.is_none()
        && ident
          .ident
          .to_string()
          .starts_with(|c: char| c.is_ascii_uppercase());
      if !constant {
        names.push(ident.ident.clone());
      }
      if let Some((_, sub)) = &ident.subpat {
        collect_bindings(sub, names);
      }
    }
    Pat::Or(or) => or
      .cases
      .iter()
      .for_each(|case| collect_bindings(case, names)),
    Pat::Paren(paren) => collect_bindings(&paren.pat, names),
    Pat::Reference(reference) => collect_bindings(&reference.pat, names),
    Pat::Slice(slice) => slice
      .elems
      .iter()
      .for_each(|elem| collect_bindings(elem, names)),
    Pat::Struct(strukt) => strukt
      .fields
      .iter()
      .for_each(|field| collect_bindings(&field.pat, names)),
    Pat::Tuple(tuple) => tuple
      .elems
      .iter()
      .for_each(|elem| collect_bindings(elem, names)),
    Pat::TupleStruct(tuple) => tuple
      .elems
      .iter()
      .for_each(|elem| collect_bindings(elem, names)),
    Pat::Type(typed) => collect_bindings(&typed.pat, names),
    _ => {}
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
