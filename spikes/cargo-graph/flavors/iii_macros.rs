//! Option (iii): one procedural macro whose output depends on LOCUS_FLAVOR,
//! which the driver sets before each compile. Input: `$crate; place, place, ...`.
use proc_macro::{Delimiter, Group, Spacing, TokenStream, TokenTree};

#[proc_macro]
pub fn prove(input: TokenStream) -> TokenStream {
    let mut tokens = input.into_iter();
    let krate: TokenStream = tokens.next().into_iter().collect(); // `$crate`
    tokens.next(); // `;`
    let check = std::env::var("LOCUS_FLAVOR").as_deref() == Ok("check");
    let path = |rest: &str| {
        let mut p = krate.clone();
        p.extend(rest.parse::<TokenStream>().unwrap());
        p
    };
    let mut out = TokenStream::new();
    if check {
        let mut place = TokenStream::new();
        for t in tokens.chain([TokenTree::from(proc_macro::Punct::new(',', Spacing::Alone))]) {
            match &t {
                TokenTree::Punct(p) if p.as_char() == ',' => {
                    if !place.is_empty() {
                        let mut arg: TokenStream = "&".parse().unwrap();
                        arg.extend([TokenTree::from(Group::new(Delimiter::Parenthesis, place))]);
                        out.extend(path("::observe"));
                        out.extend([TokenTree::from(Group::new(Delimiter::Parenthesis, arg))]);
                        out.extend(";".parse::<TokenStream>().unwrap());
                    }
                    place = TokenStream::new();
                }
                _ => place.extend([t]),
            }
        }
    }
    out.extend(path("::Erased"));
    [TokenTree::from(Group::new(Delimiter::Brace, out))].into_iter().collect()
}
