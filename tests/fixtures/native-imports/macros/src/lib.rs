use proc_macro::TokenStream;
#[proc_macro]
pub fn passthrough(input: TokenStream) -> TokenStream { input }
