extern crate proc_macro;
use proc_macro2::TokenStream as TS;
use quote::quote;

use crate::ast::{AttributeValue, Effect, Node};

pub fn build_node(node: Node) -> TS {
  match node {
    Node::Expr(expr) => {
      let text = quote! { #expr };
      quote! {{
          gloo_utils::document().create_text_node(#text)
      }}
    }
    Node::EffectDiv(Effect { ctx, signals, expr }) => {
      let ctx = quote! { #ctx };
      let inner_html = quote! {};
      quote! {{
          let node = gloo_utils::document().create_element("span")?;
          let n = node.clone();
          #(
              let #signals = #signals.clone();
          )*
          #ctx.create_effect(move || {
            let inner_html = { #expr };
            n.set_inner_html(&inner_html);
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
          AttributeValue::Value(val) => attributes.push(quote! {el.set_attribute(#key, #val)?;}),
          AttributeValue::Expr(val) => {
            let value = quote! { #val };
            attributes.push(quote! {el.set_attribute(#key, #value)?;})
          }
          AttributeValue::Effect(Effect { ctx, signals, expr }) => attributes.push(quote! {{
              let el = el.clone();
              #(
                  let #signals = #signals.clone();
              )*
              #ctx.create_effect(move || {
                  let value = { #expr };
                  el.set_attribute(#key, #expr).expect("Cannot setup attributes inside the effect");
              });
          }}),
          AttributeValue::Event(expr) => attributes.push(quote! {
            let cl =  Closure::wrap(Box::new(#expr) as Box<dyn FnMut(web_sys::Event)>);
            el.add_event_listener_with_callback(#key, cl.as_ref().unchecked_ref())?;
            cl.forget();
          }),
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
