extern crate proc_macro;
use proc_macro2::TokenStream as TS;
use quote::quote;

use crate::ast::Node;

pub fn build_node(node: Node) -> TS {
  match node {
    Node::Expr(expr) => {
      let text = quote! { #expr };
      quote! {{
          fluid::document()?.create_text_node(#text)
      }}
    }
    Node::Effect((ctx, expr)) => {
      let ctx = quote! { #ctx };
      let inner = quote! { #expr };
      quote! {{
          let node = fluid::document()?.create_element("span")?;
          let n = node.clone();
          #ctx.create_effect(move || {
            n.set_inner_html(#inner);
          });
          node
      }}
    }
    Node::Text(text) => quote! {
      {
        fluid::document()?.create_text_node(#text)
      }
    },
    Node::Element(el) => {
      let name = el.name;
      let mut attributes = Vec::new();
      let mut children = Vec::new();
      for (key, value) in el.attributes {
        attributes.push(quote! {#key, #value});
      }
      for child in el.children {
        let child_node = build_node(*child);
        children.push(child_node);
      }
      quote! {
          {
            let el = fluid::create_element(#name)?;
            #(el.set_attribute(#attributes)?;)*
            #(
                let child = #children;
                el.append_child(&child)?;
            )*
            el
          }
      }
    }
  }
}
