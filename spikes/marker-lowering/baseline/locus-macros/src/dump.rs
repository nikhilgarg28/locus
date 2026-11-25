// A canonical text form of a token stream, one token per line, shared by the
// `__lexdump!` macro (tokens from rustc's lexer) and the `tokenize` binary
// (tokens from proc_macro2's own lexer), so the two can be compared.
use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};

pub fn dump(ts: TokenStream) -> String {
    let mut out = String::new();
    walk(ts, 0, &mut out);
    out
}

fn walk(ts: TokenStream, depth: usize, out: &mut String) {
    for t in ts {
        for _ in 0..depth {
            out.push_str("  ");
        }
        match t {
            TokenTree::Ident(i) => out.push_str(&format!("Ident {i}\n")),
            TokenTree::Punct(p) => {
                let s = match p.spacing() {
                    Spacing::Joint => "Joint",
                    Spacing::Alone => "Alone",
                };
                out.push_str(&format!("Punct {} {s}\n", p.as_char()));
            }
            TokenTree::Literal(l) => out.push_str(&format!("Literal {l}\n")),
            TokenTree::Group(g) => {
                let d = match g.delimiter() {
                    Delimiter::Parenthesis => "()",
                    Delimiter::Brace => "{}",
                    Delimiter::Bracket => "[]",
                    Delimiter::None => "none",
                };
                out.push_str(&format!("Group {d}\n"));
                walk(g.stream(), depth + 1, out);
            }
        }
    }
}
