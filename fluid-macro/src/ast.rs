extern crate proc_macro;
use proc_macro2::TokenStream as TokenStream2;
use std::{collections::HashMap, error::Error};
use syn::{
  braced, bracketed, parenthesized,
  parse::{Parse, ParseStream},
  token::{Brace, Bracket, Paren},
  Ident, LitStr, Token,
};

pub enum Attribute {
  Value(String),
  Expr(TokenStream2),
  Effect((TokenStream2, TokenStream2)),
  Event(TokenStream2),
}

// impl Parse for Attribute {
// }

pub struct Element {
  pub name: String,
  pub attributes: HashMap<String, Attribute>,
  pub children: Vec<Box<Node>>,
}

pub enum Node {
  Expr(TokenStream2),
  EffectDiv((TokenStream2, TokenStream2)),
  Text(String),
  Element(Element),
}

impl Parse for Node {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    // init children and attributes
    let mut children: Vec<Box<Node>> = Vec::new();
    let mut attributes: HashMap<String, Attribute> = HashMap::new();

    let lookahead = input.lookahead1();

    // If it's { "Some String" }
    if lookahead.peek(LitStr) {
      let lit: LitStr = input.parse()?;
      return Ok(Node::Text(lit.value()));
    }
    // If it's { (rust code) }
    if lookahead.peek(Paren) {
      let expr = exhaust_paren(&input)?;
      return Ok(Node::Expr(expr));
    }
    // If it's { [ rust code ] }
    if lookahead.peek(Bracket) {
      let expr = exhause_bracket(&input)?;
      let ctx = expr.clone().into_iter().take(1).collect();
      let expr = expr.into_iter().skip(2).collect();
      return Ok(Node::EffectDiv((ctx, expr)));
    }

    // Parse ident
    let name = input.parse::<Ident>()?.to_string();

    // Parse attributes
    let lookahead = input.lookahead1();
    while !lookahead.peek(Brace) {
      // handle @event
      let lookahead = input.lookahead1();
      let mut event = false;
      if lookahead.peek(Token![@]) {
        let _: Token![@] = input.parse()?;
        event = true;
      }
      // parse attribute name
      let attribute = input.parse::<Ident>()?.to_string();

      // parse [=]
      let lookahead = input.lookahead1();
      if !lookahead.peek(Token![=]) {
        lookahead.error();
      }
      let _: Token![=] = input.parse()?;
      let lookahead = input.lookahead1();
      if event {
        let expr = exhaust_paren(&input)?;
        attributes.insert(attribute, Attribute::Event(expr));
      // If it's  "Some String"
      } else if lookahead.peek(LitStr) {
        let lit: LitStr = input.parse()?;
        let value = lit.value();
        attributes.insert(attribute, Attribute::Value(value));
      // If it's ( rust code )
      } else if lookahead.peek(Paren) {
        let expr = exhaust_paren(&input)?;
        attributes.insert(attribute, Attribute::Expr(expr));
      // If it's [ rust code ]
      } else if lookahead.peek(Bracket) {
        let expr = exhause_bracket(&input)?;
        let ctx = expr.clone().into_iter().take(1).collect();
        let expr = expr.into_iter().skip(2).collect();
        attributes.insert(attribute, Attribute::Effect((ctx, expr)));
      // parse event
      } else {
        lookahead.error();
      }
      if input.lookahead1().peek(Brace) {
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

fn exhause_bracket(input: &ParseStream) -> Result<proc_macro2::TokenStream, syn::Error> {
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
  Ok(expr)
}

fn exhaust_paren(input: &ParseStream) -> Result<proc_macro2::TokenStream, syn::Error> {
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
  Ok(expr)
}
