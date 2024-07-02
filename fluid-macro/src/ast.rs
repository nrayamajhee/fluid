extern crate proc_macro;
use proc_macro::Delimiter;
use proc_macro2::{TokenStream, TokenTree};
use std::{collections::HashMap, error::Error};
use syn::{
  braced, bracketed, parenthesized,
  parse::{Parse, ParseBuffer, ParseStream},
  token::{Brace, Bracket, Paren},
  Ident, LitStr, Token,
};

pub struct Effect {
  pub ctx: Ident,
  pub signals: Vec<Ident>,
  pub expr: TokenStream,
}

pub enum AttributeValue {
  Value(String),
  Expr(TokenStream),
  Effect(Effect),
  Event(TokenStream),
}

pub struct Element {
  pub name: String,
  pub attributes: HashMap<String, AttributeValue>,
  pub children: Vec<Box<Node>>,
}

impl Parse for Effect {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let content;
    bracketed!(content in input);
    let ctx = content.parse()?;
    ignore_token::<Token![,]>(&&content)?;
    let sigs;
    bracketed!(sigs in content);
    ignore_token::<Token![->]>(&&content)?;
    let expr = content.parse()?;
    let mut signals = Vec::new();
    for _ in 0..sigs.cursor().token_stream().into_iter().count() {
      signals.push(sigs.parse()?);
    }
    Ok(Self { ctx, signals, expr })
  }
}

pub enum Node {
  Expr(TokenStream),
  EffectDiv(Effect),
  Text(String),
  Element(Element),
}

impl Parse for Node {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    // init children and attributes
    let mut children: Vec<Box<Node>> = Vec::new();
    let mut attributes: HashMap<String, AttributeValue> = HashMap::new();

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
      let effect: Effect = input.parse()?;
      return Ok(Node::EffectDiv(effect));
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
        ignore_token::<Token![@]>(&input)?;
        event = true;
      }
      // parse attribute name
      let attribute = input.parse::<Ident>()?.to_string();

      // parse [=]
      let lookahead = input.lookahead1();
      if !lookahead.peek(Token![=]) {
        lookahead.error();
      }
      ignore_token::<Token![=]>(&input)?;
      let lookahead = input.lookahead1();
      if event {
        let expr = exhaust_paren(&input)?;
        attributes.insert(attribute, AttributeValue::Event(expr));
      // If it's  "Some String"
      } else if lookahead.peek(LitStr) {
        let lit: LitStr = input.parse()?;
        let value = lit.value();
        attributes.insert(attribute, AttributeValue::Value(value));
      // If it's ( rust code )
      } else if lookahead.peek(Paren) {
        let expr = exhaust_paren(&input)?;
        attributes.insert(attribute, AttributeValue::Expr(expr));
      // If it's [ rust code ]
      } else if lookahead.peek(Bracket) {
        let effect: Effect = input.parse()?;
        attributes.insert(attribute, AttributeValue::Effect(effect));
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

fn exhause_bracket(input: &ParseStream) -> Result<TokenStream, syn::Error> {
  let content;
  bracketed!(content in input);
  let expr = content.cursor().token_stream();
  while !content.is_empty() {
    step(&content)?;
  }
  Ok(expr)
}

fn exhaust_paren(input: &ParseStream) -> Result<TokenStream, syn::Error> {
  let content;
  parenthesized!(content in input);
  let expr = content.cursor().token_stream();
  while !content.is_empty() {
    step(&content)?;
  }
  Ok(expr)
}

fn ignore_token<T: Parse>(input: &ParseStream) -> syn::Result<()> {
  let _: T = input.parse()?;
  Ok(())
}

fn step(content: &ParseBuffer) -> syn::Result<()> {
  content.step(|cursor| {
    if let Some((_, next)) = cursor.token_tree() {
      return Ok(((), next));
    } else {
      return Err(cursor.error("Something went wrong parsing contents inside ()!"));
    }
  })?;
  Ok(())
}
