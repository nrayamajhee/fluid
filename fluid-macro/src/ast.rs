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

pub enum AttributeType {
  Value(String),
  Effect((TokenStream2, TokenStream2)),
  Event(TokenStream2),
}

pub struct Element {
  pub name: String,
  pub attributes: HashMap<String, AttributeType>,
  pub children: Vec<Box<Node>>,
}

pub enum Node {
  Expr(TokenStream2),
  EffectDiv((TokenStream2, TokenStream2)),
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
    // return effect node
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
      let expr = expr.into_iter().skip(2).collect();
      return Ok(Node::EffectDiv((ctx, expr)));
    }
    // Parse ident and attributes
    let ident: Ident = input.parse()?;
    let name = ident.to_string();
    let mut children: Vec<Box<Node>> = Vec::new();
    let mut attributes: HashMap<String, AttributeType> = HashMap::new();
    let lookahead = input.lookahead1();
    while !lookahead.peek(Brace) {
      let lookahead = input.lookahead1();
      let mut event = false;
      if lookahead.peek(Token![@]) {
        let _: Token![@] = input.parse()?;
        event = true;
      }
      let ident: Ident = input.parse()?;
      let attribute = ident.to_string();
      let lookahead = input.lookahead1();
      if !lookahead.peek(Token![=]) {
        lookahead.error();
      }
      let _: Token![=] = input.parse()?;
      if event {
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
        attributes.insert(attribute, AttributeType::Event(expr));
      } else {
        let lookahead = input.lookahead1();
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
          let expr = expr.into_iter().skip(2).collect();
          attributes.insert(attribute, AttributeType::Effect((ctx, expr)));
        } else if lookahead.peek(LitStr) {
          let lit: LitStr = input.parse()?;
          let value = lit.value();
          attributes.insert(attribute, AttributeType::Value(value));
        } else {
          lookahead.error();
        }
      }
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
