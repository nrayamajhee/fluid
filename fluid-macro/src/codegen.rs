extern crate proc_macro;
use proc_macro2::TokenStream as TS;
use quote::quote;

use crate::ast::{AttributeType, Node};

pub fn build_node(node: Node) -> TS {
  match node {
    Node::Expr(expr) => {
      let text = quote! { #expr };
      quote! {{
          gloo_utils::document().create_text_node(#text)
      }}
    }
    Node::EffectDiv((ctx, expr)) => {
      let ctx = quote! { #ctx };
      let inner = quote! { #expr };
      quote! {{
          let node = gloo_utils::document().create_element("span")?;
          let n = node.clone();
          #ctx.create_effect(move || {
            n.set_inner_html(#inner);
          });
          node
      }}
    }
    Node::Text(text) => quote! {
      {
        gloo_utils::document().create_text_node(#text)
      }
    },
    Node::Element(el) => {
      let name = el.name;
      let mut attributes = Vec::new();
      let mut children = Vec::new();
      for (key, value) in el.attributes {
        match value {
          AttributeType::Value(val) => attributes.push(quote! {el.set_attribute(#key, #val)?;}),
          AttributeType::Effect((ctx, expr)) => attributes.push(quote! {{
              let el = el.clone();
              #ctx.create_effect(move || {
                  el.set_attribute(#key, #expr).expect("Cannot setup attributes inside the effect");
              });
          }}),
        }
      }
      for child in el.children {
        let child_node = build_node(*child);
        children.push(child_node);
      }
      quote! {
          {
            let el = gloo_utils::document().create_element(#name)?;
            #(#attributes)*
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
