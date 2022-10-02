extern crate proc_macro;
use proc_macro::TokenStream;
use syn::parse_macro_input;

mod ast;
use ast::Node;

mod codegen;

#[proc_macro]
pub fn html(item: TokenStream) -> TokenStream {
  let node = parse_macro_input!(item as Node);
  if let Node::Text(_) = node {
    assert!(false, "Cannot build DOM tree with text node at the root");
    "()".parse().unwrap()
  } else {
    codegen::build_node(node).into()
  }
}
