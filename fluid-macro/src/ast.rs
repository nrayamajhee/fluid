use proc_macro2::{Span, TokenStream};
use syn::{
  braced,
  ext::IdentExt,
  parse::{Parse, ParseStream},
  spanned::Spanned,
  token::Brace,
  Expr, Ident, LitInt, LitStr, Pat, Token, Type,
};

/// A `|a, b| expr` body: the names are cloned into the closure.
pub struct Reactive {
  pub captures: Vec<Ident>,
  pub expr: TokenStream,
}

pub enum AttrName {
  Plain(String),
  /// `class:name`: toggles the class(es).
  Class(String),
  /// `prop:name`: sets a DOM property.
  Prop(String),
}

pub enum AttrValue {
  Lit(LitStr),
  Expr(TokenStream),
  /// `{ move || expr }`: re-applied whenever what it reads changes.
  Closure(TokenStream),
  /// `#{ |sigs| expr }`.
  Reactive(Reactive),
}

pub enum Attr {
  Attr {
    name: AttrName,
    span: Span,
    value: AttrValue,
  },
  Event {
    name: String,
    modifiers: Vec<Ident>,
    handler: TokenStream,
    /// The type annotated on the handler's first parameter.
    event_type: Option<Box<Type>>,
  },
}

pub struct Element {
  pub name: String,
  pub span: Span,
  pub attrs: Vec<Attr>,
  pub children: Vec<Node>,
}

pub enum Node {
  /// `"text"`
  Text(LitStr),
  /// `{ expr }`: a node, or text from anything `Display`.
  Expr(TokenStream),
  /// `{ move || expr }`
  Closure(TokenStream),
  /// `#{ |sigs| expr }` (legacy, owned by `ctx`) / `#html{ |sigs| expr }`:
  /// raw HTML in a `<span>`.
  Html {
    reactive: Reactive,
    legacy: bool,
  },
  /// `#text{ |sigs| expr }`
  ReactiveText(Reactive),
  Element(Element),
  /// `if |sigs| cond { .. } else if cond { .. } else { .. }`
  If {
    captures: Vec<Ident>,
    branches: Vec<(Expr, Vec<Node>)>,
    otherwise: Option<Vec<Node>>,
  },
  /// `match |sigs| expr { pat if guard => { .. } }`
  Match {
    captures: Vec<Ident>,
    expr: Box<Expr>,
    arms: Vec<Arm>,
  },
  /// `for |sigs| pat in items, key = expr { .. }`
  For {
    captures: Vec<Ident>,
    pat: Box<Pat>,
    items: Box<Expr>,
    key: Box<Expr>,
    children: Vec<Node>,
  },
}

pub struct Arm {
  pub pat: Pat,
  pub guard: Option<Expr>,
  pub children: Vec<Node>,
}

impl Node {
  pub fn is_control_flow(&self) -> bool {
    matches!(
      self,
      Node::If { .. } | Node::Match { .. } | Node::For { .. }
    )
  }
}

impl Parse for Node {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    if input.peek(LitStr) {
      return Ok(Node::Text(input.parse()?));
    }

    if input.peek(Token![#]) {
      input.parse::<Token![#]>()?;
      let kind = if input.peek(Ident) {
        let kind: Ident = input.parse()?;
        match kind.to_string().as_str() {
          "text" | "html" => Some(kind),
          _ => {
            return Err(syn::Error::new(
              kind.span(),
              "expected `#{ |..| }`, `#text{ |..| }` or `#html{ |..| }`",
            ))
          }
        }
      } else {
        None
      };
      let content;
      braced!(content in input);
      let reactive = parse_reactive(&content)?;
      return Ok(match kind {
        Some(kind) if kind == "text" => Node::ReactiveText(reactive),
        Some(_) => Node::Html {
          reactive,
          legacy: false,
        },
        None => Node::Html {
          reactive,
          legacy: true,
        },
      });
    }

    if input.peek(Brace) {
      let content;
      braced!(content in input);
      let expr: TokenStream = content.parse()?;
      if is_closure_literal(&expr) {
        return Ok(Node::Closure(expr));
      }
      return Ok(Node::Expr(expr));
    }

    if input.peek(Token![if]) {
      input.parse::<Token![if]>()?;
      let captures = parse_captures(input)?;
      let mut branches = vec![(
        Expr::parse_without_eager_brace(input)?,
        parse_children(input)?,
      )];
      let mut otherwise = None;
      while input.peek(Token![else]) {
        input.parse::<Token![else]>()?;
        if input.peek(Token![if]) {
          input.parse::<Token![if]>()?;
          if input.peek(Token![|]) || input.peek(Token![||]) {
            return Err(
              input.error("only the first `if` takes a capture list; it covers every branch"),
            );
          }
          branches.push((
            Expr::parse_without_eager_brace(input)?,
            parse_children(input)?,
          ));
        } else {
          otherwise = Some(parse_children(input)?);
          break;
        }
      }
      return Ok(Node::If {
        captures,
        branches,
        otherwise,
      });
    }

    if input.peek(Token![match]) {
      input.parse::<Token![match]>()?;
      let captures = parse_captures(input)?;
      let expr = Expr::parse_without_eager_brace(input)?;
      let content;
      braced!(content in input);
      let mut arms = Vec::new();
      while !content.is_empty() {
        let pat = Pat::parse_multi_with_leading_vert(&content)?;
        let guard = if content.peek(Token![if]) {
          content.parse::<Token![if]>()?;
          Some(content.parse()?)
        } else {
          None
        };
        content.parse::<Token![=>]>()?;
        if !content.peek(Brace) {
          return Err(content.error("expected `{ .. }` with the arm's nodes"));
        }
        let children = parse_children(&content)?;
        if content.peek(Token![,]) {
          content.parse::<Token![,]>()?;
        }
        arms.push(Arm {
          pat,
          guard,
          children,
        });
      }
      return Ok(Node::Match {
        captures,
        expr: Box::new(expr),
        arms,
      });
    }

    if input.peek(Token![for]) {
      input.parse::<Token![for]>()?;
      let captures = parse_captures(input)?;
      let pat = Pat::parse_single(input)?;
      input.parse::<Token![in]>()?;
      let items = Expr::parse_without_eager_brace(input)?;
      let key_error = "a list needs a key: `for item in items, key = item.id { .. }`";
      input
        .parse::<Token![,]>()
        .map_err(|err| syn::Error::new(err.span(), key_error))?;
      let key_word = Ident::parse_any(input)?;
      if key_word != "key" {
        return Err(syn::Error::new(key_word.span(), key_error));
      }
      input.parse::<Token![=]>()?;
      let key = Expr::parse_without_eager_brace(input)?;
      let children = parse_children(input)?;
      return Ok(Node::For {
        captures,
        pat: Box::new(pat),
        items: Box::new(items),
        key: Box::new(key),
        children,
      });
    }

    Ok(Node::Element(input.parse()?))
  }
}

impl Parse for Element {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let span = input.span();
    if !input.peek(Ident) {
      return Err(input.error("expected an element, a string literal or a braced expression"));
    }
    let name = parse_dashed(input)?;
    validate_name(&name, span, "tag")?;

    let mut attrs = Vec::new();
    while !input.peek(Brace) {
      if input.is_empty() {
        return Err(input.error(format!("expected `{{` with the children of `{name}`")));
      }
      attrs.push(input.parse()?);
    }
    check_attrs(&attrs)?;
    let children = parse_children(input)?;
    Ok(Element {
      name,
      span,
      attrs,
      children,
    })
  }
}

impl Parse for Attr {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    if input.peek(Token![@]) {
      input.parse::<Token![@]>()?;
      let name = parse_dashed(input)?;
      let mut modifiers = Vec::new();
      while input.peek(Token![.]) {
        input.parse::<Token![.]>()?;
        let modifier = Ident::parse_any(input)?;
        if !matches!(
          modifier.to_string().as_str(),
          "capture" | "once" | "passive" | "nonpassive" | "prevent" | "stop"
        ) {
          return Err(syn::Error::new(
            modifier.span(),
            "expected one of `capture`, `once`, `passive`, `nonpassive`, `prevent`, `stop`",
          ));
        }
        modifiers.push(modifier);
      }
      let has = |name: &str| modifiers.iter().any(|modifier| modifier == name);
      if has("passive") && has("nonpassive") {
        return Err(input.error("an event can't be both `passive` and `nonpassive`"));
      }
      if has("passive") && has("prevent") {
        return Err(input.error("`prevent` has no effect on a `passive` listener"));
      }
      input
        .parse::<Token![=]>()
        .map_err(|err| syn::Error::new(err.span(), "expected `=` after the event name"))?;
      if !input.peek(Brace) {
        return Err(input.error("expected a braced handler, e.g. `@click={ move |_| .. }`"));
      }
      let content;
      braced!(content in input);
      let handler: TokenStream = content.parse()?;
      let event_type = match syn::parse2::<Expr>(handler.clone()) {
        Ok(Expr::Closure(closure)) => closure.inputs.first().and_then(|input| match input {
          syn::Pat::Type(typed) => Some(typed.ty.clone()),
          _ => None,
        }),
        _ => None,
      };
      return Ok(Attr::Event {
        name,
        modifiers,
        handler,
        event_type,
      });
    }

    let span = input.span();
    let name = if input.peek(LitStr) {
      AttrName::Plain(input.parse::<LitStr>()?.value())
    } else {
      let first = parse_dashed(input)?;
      if input.peek(Token![:]) {
        input.parse::<Token![:]>()?;
        let rest = if input.peek(LitStr) {
          input.parse::<LitStr>()?.value()
        } else {
          parse_dashed(input)?
        };
        match first.as_str() {
          "class" => AttrName::Class(rest),
          "prop" => AttrName::Prop(rest),
          _ => AttrName::Plain(format!("{first}:{rest}")),
        }
      } else {
        AttrName::Plain(first)
      }
    };
    match &name {
      AttrName::Plain(name) | AttrName::Prop(name) => validate_name(name, span, "attribute")?,
      AttrName::Class(classes) => {
        if classes.split_whitespace().next().is_none() {
          return Err(syn::Error::new(span, "a class toggle needs a class name"));
        }
      }
    }

    input
      .parse::<Token![=]>()
      .map_err(|err| syn::Error::new(err.span(), "expected `=` after the attribute name"))?;

    let value = if input.peek(LitStr) {
      AttrValue::Lit(input.parse()?)
    } else if input.peek(Token![#]) {
      input.parse::<Token![#]>()?;
      let content;
      braced!(content in input);
      AttrValue::Reactive(parse_reactive(&content)?)
    } else if input.peek(Brace) {
      let content;
      braced!(content in input);
      let expr: TokenStream = content.parse()?;
      if is_closure_literal(&expr) {
        AttrValue::Closure(expr)
      } else {
        AttrValue::Expr(expr)
      }
    } else {
      return Err(input.error(
        "expected a string literal, `{ expr }` or `#{ |..| expr }` for the attribute value",
      ));
    };
    if let (AttrName::Class(_), AttrValue::Lit(lit)) = (&name, &value) {
      return Err(syn::Error::new(
        lit.span(),
        "a class toggle takes a `bool`, e.g. `class:hidden={ closed }`",
      ));
    }
    Ok(Attr::Attr { name, span, value })
  }
}

/// `ident(-ident)*`, keywords allowed (`type`, `for`), e.g. `aria-label`.
fn parse_dashed(input: ParseStream) -> syn::Result<String> {
  let mut name = Ident::parse_any(input)?.unraw().to_string();
  while input.peek(Token![-]) {
    input.parse::<Token![-]>()?;
    name.push('-');
    if input.peek(LitInt) {
      name.push_str(&input.parse::<LitInt>()?.to_string());
    } else {
      name.push_str(&Ident::parse_any(input)?.unraw().to_string());
    }
  }
  Ok(name)
}

/// What the DOM's `createElement`/`setAttribute` accept, minus oddities.
fn validate_name(name: &str, span: Span, what: &str) -> syn::Result<()> {
  let valid = name
    .chars()
    .next()
    .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
    && name
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'));
  if valid {
    Ok(())
  } else {
    Err(syn::Error::new(
      span,
      format!("`{name}` is not a valid {what} name"),
    ))
  }
}

fn check_attrs(attrs: &[Attr]) -> syn::Result<()> {
  let mut seen: Vec<String> = Vec::new();
  let mut reactive_class = false;
  let mut toggles = false;
  for attr in attrs {
    let Attr::Attr { name, span, value } = attr else {
      continue;
    };
    let key = match name {
      AttrName::Plain(name) => name.clone(),
      AttrName::Class(name) => format!("class:{name}"),
      AttrName::Prop(name) => format!("prop:{name}"),
    };
    if seen.contains(&key) {
      return Err(syn::Error::new(
        *span,
        format!("duplicate attribute `{key}`"),
      ));
    }
    seen.push(key);
    match (name, value) {
      (AttrName::Plain(name), AttrValue::Reactive(_) | AttrValue::Closure(_))
        if name == "class" =>
      {
        reactive_class = true
      }
      (AttrName::Class(_), _) => toggles = true,
      _ => {}
    }
    if reactive_class && toggles {
      return Err(syn::Error::new(
        *span,
        "a reactive `class` would overwrite `class:` toggles; use a static `class` with toggles",
      ));
    }
  }
  Ok(())
}

fn parse_children(input: ParseStream) -> syn::Result<Vec<Node>> {
  let content;
  braced!(content in input);
  let mut children = Vec::new();
  while !content.is_empty() {
    children.push(content.parse()?);
  }
  Ok(children)
}

/// An optional `|a, b|` before a control-flow expression.
fn parse_captures(input: ParseStream) -> syn::Result<Vec<Ident>> {
  let mut captures = Vec::new();
  if input.peek(Token![||]) {
    input.parse::<Token![||]>()?;
  } else if input.peek(Token![|]) {
    input.parse::<Token![|]>()?;
    while !input.peek(Token![|]) {
      captures.push(input.parse::<Ident>()?);
      if input.peek(Token![,]) {
        input.parse::<Token![,]>()?;
      } else if !input.peek(Token![|]) {
        return Err(input.error("expected `,` or `|` in the capture list"));
      }
    }
    input.parse::<Token![|]>()?;
  }
  Ok(captures)
}

/// `|a, b| expr` (or `|| expr`) inside a `#{ }`.
fn parse_reactive(content: ParseStream) -> syn::Result<Reactive> {
  if !(content.peek(Token![|]) || content.peek(Token![||])) {
    return Err(content.error("expected '|' to start signal list in reactive block"));
  }
  let captures = parse_captures(content)?;
  let expr: TokenStream = content.parse()?;
  if expr.is_empty() {
    return Err(syn::Error::new(
      expr.span(),
      "expected an expression after the capture list",
    ));
  }
  Ok(Reactive { captures, expr })
}

/// `move || expr`, or a block ending in one (`{ let a = a.clone(); move || .. }`).
fn is_closure_literal(tokens: &TokenStream) -> bool {
  fn check(expr: &Expr) -> bool {
    match expr {
      Expr::Closure(closure) => closure.inputs.is_empty(),
      Expr::Block(block) => matches!(
        block.block.stmts.last(),
        Some(syn::Stmt::Expr(expr, None)) if check(expr)
      ),
      _ => false,
    }
  }
  syn::parse2::<Expr>(tokens.clone()).is_ok_and(|expr| check(&expr))
}
