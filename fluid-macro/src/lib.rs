extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use std::collections::HashMap;
use syn::{braced, parse_macro_input};

enum Node {
  Text(String),
  Element(Element),
}

struct Element {
  pub name: String,
  attributes: HashMap<String, String>,
  children: Vec<Box<Node>>,
}

use syn::{
  parse::{Parse, ParseStream},
  token::Brace,
  Ident, LitStr, Result, Token,
};

impl Parse for Node {
  fn parse(input: ParseStream) -> Result<Self> {
    let lookahead = input.lookahead1();
    if lookahead.peek(LitStr) {
      let lit: LitStr = input.parse()?;
      return Ok(Node::Text(lit.value()));
    }
    let ident: Ident = input.parse()?;
    let name = ident.to_string();
    let mut children: Vec<Box<Node>> = Vec::new();
    let mut attributes: HashMap<String, String> = HashMap::new();
    let lookahead = input.lookahead1();
    while !lookahead.peek(Brace) {
      let ident: Ident = input.parse()?;
      let attribute = ident.to_string();
      let lookahead = input.lookahead1();
      if !lookahead.peek(Token![=]) {
        lookahead.error();
      }
      let _: Token![=] = input.parse()?;
      let lookahead = input.lookahead1();
      if !lookahead.peek(LitStr) {
        lookahead.error();
      }
      let lit: LitStr = input.parse()?;
      let value = lit.value();
      attributes.insert(attribute, value);
      let lookahead = input.lookahead1();
      if lookahead.peek(Brace) {
        break;
      }
    }
    let content;
    let _: Brace = braced!(content in input);
    while !content.is_empty() {
      let child: Node = content.parse()?;
      children.push(Box::new(child));
    }
    Ok(Node::Element(Element {
      name,
      attributes,
      children,
    }))
  }
}

use proc_macro2::TokenStream as TS;

fn build_node(node: Node) -> TS {
  match node {
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

#[proc_macro]
pub fn html(item: TokenStream) -> TokenStream {
  let node = parse_macro_input!(item as Node);
  if let Node::Text(_) = node {
    assert!(false, "Cannot build DOM tree with text node at the root");
    "()".parse().unwrap()
  } else {
    build_node(node).into()
  }
}
