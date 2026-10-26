//! The external review's counterexample: a deterministic attribute macro that
//! replaces a function's final expression `x` with `x + 1` when the function
//! contains no binding named `h`.
extern crate proc_macro;
use proc_macro::{Delimiter, Group, Literal, Punct, Spacing, TokenStream, TokenTree};

#[proc_macro_attribute]
pub fn token_sensitive(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut toks: Vec<TokenTree> = item.into_iter().collect();
    let Some(TokenTree::Group(body)) = toks.pop() else { panic!("expected a function") };
    let mut inner: Vec<TokenTree> = body.stream().into_iter().collect();
    if !binds_h(body.stream()) {
        let last = inner.pop().expect("a final expression");
        let plus_one: TokenStream = [
            last,
            TokenTree::Punct(Punct::new('+', Spacing::Alone)),
            TokenTree::Literal(Literal::u32_unsuffixed(1)),
        ]
        .into_iter()
        .collect();
        inner.push(TokenTree::Group(Group::new(Delimiter::Parenthesis, plus_one)));
    }
    toks.push(TokenTree::Group(Group::new(Delimiter::Brace, inner.into_iter().collect())));
    toks.into_iter().collect()
}

fn binds_h(ts: TokenStream) -> bool {
    let toks: Vec<TokenTree> = ts.into_iter().collect();
    toks.windows(2).any(|w| matches!(w, [TokenTree::Ident(a), TokenTree::Ident(b)] if a.to_string() == "let" && b.to_string() == "h"))
        || toks.iter().any(|t| matches!(t, TokenTree::Group(g) if binds_h(g.stream())))
}
