extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use std::collections::HashMap;
use syn::{braced, parse_macro_input};

struct Node {
  pub name: String,
  // text: String,
  attributes: HashMap<String, String>,
  // children: Vec<Box<Node>>,
}

use syn::{
  parse::{Parse, ParseStream},
  token::Brace,
  Ident, LitStr, Result, Token,
};

impl Parse for Node {
  fn parse(input: ParseStream) -> Result<Self> {
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
      let value: LitStr = input.parse()?;
      let value = value.value();
      attributes.insert(attribute, value);
      let lookahead = input.lookahead1();
      if lookahead.peek(Brace) {
        break;
      }
    }
    // let mut text = String::new();
    let content;
    let _: Brace = braced!(content in input);
    if !content.is_empty() {
      let lookahead = content.lookahead1();
      if lookahead.peek(LitStr) {
        let _: LitStr = content.parse()?;
        // text = lit.value();
      } else {
        let child: Node = content.parse()?;
        children.push(Box::new(child));
      }
    }
    Ok(Node {
      name,
      attributes,
      // children,
      // text,
    })
  }
}

#[proc_macro]
pub fn html(item: TokenStream) -> TokenStream {
  let node = parse_macro_input!(item as Node);
  let name = node.name;
  let mut attributes = Vec::new();
  for (key, value) in node.attributes {
    attributes.push(quote! {#key, #value});
  }
  quote! {
      {
        let el = fluid::create_element(#name)?;
        #(el.set_attribute(#attributes)?;)*
        el
      }
  }
  .into()
}
