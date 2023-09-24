extern crate proc_macro;
use fluid::Context;
use proc_macro2::TokenStream as TokenStream2;
use quote::ToTokens;
use std::{collections::HashMap, error::Error};
use syn::{
  braced, bracketed, parenthesized,
  parse::{Parse, ParseStream},
  token::{Brace, Bracket, Paren},
  Ident, LitStr, Result, Token,
};

pub struct Element {
  pub name: String,
  pub attributes: HashMap<String, String>,
  pub children: Vec<Box<Node>>,
}

pub enum Node {
  Expr(TokenStream2),
  Effect((TokenStream2, TokenStream2)),
  Text(String),
  Element(Element),
}

impl Parse for Node {
  fn parse(input: ParseStream) -> Result<Self> {
    let lookahead = input.lookahead1();
    // If it's { "Some String" }
    // return a text node
    if lookahead.peek(LitStr) {
      let lit: LitStr = input.parse()?;
      return Ok(Node::Text(lit.value()));
    }
    // If it's { (rust code) }
    // return expr node
    if lookahead.peek(Paren) {
      let content;
      parenthesized!(content in input);
      let expr = content.cursor().token_stream();
      while !content.is_empty() {
        content.step(|cursor| {
          if let Some((_, next)) = cursor.token_tree() {
            return Ok(((), next));
          } else {
            return Err(cursor.error("Something went wrong parsing contents inside ()!"));
          }
        })?;
      }
      return Ok(Node::Expr(expr));
    }
    // If it's { [ rust code ] }
    // create effect node
    if lookahead.peek(Bracket) {
      let content;
      bracketed!(content in input);
      let expr = content.cursor().token_stream();
      while !content.is_empty() {
        content.step(|cursor| {
          if let Some((_, next)) = cursor.token_tree() {
            return Ok(((), next));
          } else {
            return Err(cursor.error("Something went wrong parsing contents inside ()!"));
          }
        })?;
      }
      let ctx = expr.clone().into_iter().take(1).collect();
      dbg!(&ctx);
      let expr = expr.into_iter().skip(2).collect();
      dbg!(&expr);
      return Ok(Node::Effect((ctx, expr)));
    }
    // Parse ident and attributes
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
    // Parse children
    let content;
    braced!(content in input);
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
