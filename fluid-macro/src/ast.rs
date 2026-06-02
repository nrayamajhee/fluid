extern crate proc_macro;
use proc_macro2::TokenStream;
use std::collections::HashMap;
use syn::{
  braced,
  parse::{Parse, ParseBuffer, ParseStream},
  token::Brace,
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

    // If it's a string literal, e.g. "Some String"
    if lookahead.peek(LitStr) {
      let lit: LitStr = input.parse()?;
      return Ok(Node::Text(lit.value()));
    }

    // If it's a reactive block, e.g. #{ |sigs| expr }
    if lookahead.peek(Token![#]) {
      ignore_token::<Token![#]>(&input)?;
      let content;
      braced!(content in input);
      
      if content.peek(Token![|]) {
        ignore_token::<Token![|]>(&content)?;
        let mut signals = Vec::new();
        while !content.peek(Token![|]) {
          let sig: Ident = content.parse()?;
          signals.push(sig);
          if content.peek(Token![,]) {
            ignore_token::<Token![,]>(&content)?;
          }
        }
        ignore_token::<Token![|]>(&content)?;
        
        let expr = content.cursor().token_stream();
        while !content.is_empty() {
          step(&content)?;
        }
        
        let ctx = Ident::new("ctx", proc_macro2::Span::call_site());
        return Ok(Node::EffectDiv(Effect { ctx, signals, expr }));
      } else {
        return Err(content.error("Expected '|' to start signal list in reactive block"));
      }
    }

    // If it's a standard braced block `{ ... }` (static expression/closure/etc.)
    if lookahead.peek(Brace) {
      let content;
      braced!(content in input);
      let expr = content.cursor().token_stream();
      while !content.is_empty() {
        step(&content)?;
      }
      return Ok(Node::Expr(expr));
    }

    // Parse ident (tag name)
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
        return Err(input.error("Expected '=' after attribute name"));
      }
      ignore_token::<Token![=]>(&input)?;
      
      let lookahead = input.lookahead1();
      if event {
        if lookahead.peek(Brace) {
          let content;
          braced!(content in input);
          let expr = content.cursor().token_stream();
          while !content.is_empty() {
            step(&content)?;
          }
          attributes.insert(attribute, AttributeValue::Event(expr));
        } else {
          return Err(input.error("Expected braced expression for event attribute value"));
        }
      } else if lookahead.peek(LitStr) {
        let lit: LitStr = input.parse()?;
        let value = lit.value();
        attributes.insert(attribute, AttributeValue::Value(value));
      } else if lookahead.peek(Token![#]) {
        ignore_token::<Token![#]>(&input)?;
        let content;
        braced!(content in input);
        if content.peek(Token![|]) {
          ignore_token::<Token![|]>(&content)?;
          let mut signals = Vec::new();
          while !content.peek(Token![|]) {
            let sig: Ident = content.parse()?;
            signals.push(sig);
            if content.peek(Token![,]) {
              ignore_token::<Token![,]>(&content)?;
            }
          }
          ignore_token::<Token![|]>(&content)?;
          
          let expr = content.cursor().token_stream();
          while !content.is_empty() {
            step(&content)?;
          }
          
          let ctx = Ident::new("ctx", proc_macro2::Span::call_site());
          attributes.insert(attribute, AttributeValue::Effect(Effect { ctx, signals, expr }));
        } else {
          return Err(content.error("Expected '|' to start signal list in reactive block"));
        }
      } else if lookahead.peek(Brace) {
        let content;
        braced!(content in input);
        let expr = content.cursor().token_stream();
        while !content.is_empty() {
          step(&content)?;
        }
        attributes.insert(attribute, AttributeValue::Expr(expr));
      } else {
        return Err(input.error("Expected string literal, reactive block, or braced expression for attribute value"));
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

fn ignore_token<T: Parse>(input: ParseStream) -> syn::Result<()> {
  let _: T = input.parse()?;
  Ok(())
}

fn step(content: &ParseBuffer) -> syn::Result<()> {
  content.step(|cursor| {
    if let Some((_, next)) = cursor.token_tree() {
      return Ok(((), next));
    } else {
      return Err(cursor.error("Something went wrong parsing braced contents!"));
    }
  })?;
  Ok(())
}
