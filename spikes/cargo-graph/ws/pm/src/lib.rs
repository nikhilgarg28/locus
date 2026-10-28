use proc_macro::TokenStream;

#[proc_macro]
pub fn answer(_: TokenStream) -> TokenStream {
    "42u32".parse().unwrap()
}
