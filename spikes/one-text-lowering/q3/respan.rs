//! An attribute macro that gives every token of its item the span of its own
//! call site, as macros that rebuild their input sometimes do. The program's
//! meaning is unchanged: call-site spans resolve names as the source does.
extern crate proc_macro;
use proc_macro::{Group, Span, TokenStream, TokenTree};

#[proc_macro_attribute]
pub fn respan(_attr: TokenStream, item: TokenStream) -> TokenStream {
    respan_all(item, Span::call_site())
}

fn respan_all(ts: TokenStream, span: Span) -> TokenStream {
    ts.into_iter()
        .map(|t| match t {
            TokenTree::Group(g) => {
                let mut n = Group::new(g.delimiter(), respan_all(g.stream(), span));
                n.set_span(span);
                TokenTree::Group(n)
            }
            mut t => {
                t.set_span(span);
                t
            }
        })
        .collect()
}
