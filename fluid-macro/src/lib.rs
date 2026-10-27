extern crate proc_macro;
use proc_macro::TokenStream;
use syn::parse_macro_input;

mod ast;
use ast::Node;

mod codegen;

#[proc_macro]
pub fn html(item: TokenStream) -> TokenStream {
  let node = parse_macro_input!(item as Node);
  if node.is_control_flow() {
    return syn::Error::new(
      proc_macro2::Span::call_site(),
      "`if`, `match` and `for` must be inside an element: `div { if .. { .. } }`",
    )
    .to_compile_error()
    .into();
  }
  codegen::build_root(node).into()
}
