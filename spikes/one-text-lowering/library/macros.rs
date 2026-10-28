//! Crate `locus_macros`: `prove!`, `logic!` and `prop!` for the invocations
//! that rustc expands, because lowering did not see them (inside another
//! macro's input or output, or through an alias). Built twice, like the
//! library: with `--cfg locus_check` it observes what the logical code
//! reads; without, it expands to the bare marker.
//!
//! The spike's question 3 is how it decides, for each root of a place, whether
//! the root is ghost (observe the root only: a ghost has no fields) or runtime
//! (observe the place precisely). `LOCUS_ROOTS` picks the answer:
//! `precise`, `whole` (a), `table` (b, reads `LOCUS_SCOPES`), `dispatch` (d),
//! and `combined`: the table for a root whose span covers its own text, and
//! dispatch for one that a macro gave another span.
//! `LOCUS_SPIKE_DEBUG=1` prints each root's span position to stderr.
#![cfg_attr(not(locus_check), allow(unused))]
extern crate proc_macro;
use proc_macro::{Delimiter, Group, Ident, Punct, Spacing, Span, TokenStream, TokenTree};

#[proc_macro]
pub fn prove(input: TokenStream) -> TokenStream {
    expand(input, "Erased")
}
#[proc_macro]
pub fn logic(input: TokenStream) -> TokenStream {
    expand(input, "ErasedData")
}
#[proc_macro]
pub fn prop(input: TokenStream) -> TokenStream {
    expand(input, "ErasedData")
}

fn path(s: &str) -> TokenStream {
    s.parse().unwrap()
}

#[cfg(not(locus_check))]
fn expand(_input: TokenStream, marker: &str) -> TokenStream {
    path(&format!("::__locus::{marker}"))
}

#[cfg(locus_check)]
fn expand(input: TokenStream, marker: &str) -> TokenStream {
    let mode = std::env::var("LOCUS_ROOTS").unwrap_or_else(|_| "precise".into());
    let mut places = Vec::new();
    find_places(input, &mut places);
    let mut body = TokenStream::new();
    for place in places {
        let TokenTree::Ident(root) = &place[0] else { unreachable!() };
        // Span::source_text is stable: a root a macro re-spanned shows other text.
        let own_text = root.span().source_text().is_some_and(|t| t == root.to_string());
        let dispatch = mode == "dispatch" || (mode == "combined" && !own_text);
        let ghost = match mode.as_str() {
            "precise" => false,
            "whole" => true,
            "table" | "combined" => !dispatch && table_says_ghost(root),
            _ => false,
        };
        if std::env::var("LOCUS_SPIKE_DEBUG").is_ok() {
            let s = root.span();
            eprintln!(
                "locus-root {root} file={} line={} column={} source_text={:?} mode={mode} ghost={ghost}",
                s.local_file().map(|p| p.display().to_string()).unwrap_or_default(),
                s.line(),
                s.column(),
                s.source_text().unwrap_or_default()
            );
        }
        // The borrow's `&` gets the root's span, so a borrow error points at
        // the place where the program wrote it.
        let mut amp = Punct::new('&', Spacing::Alone);
        amp.set_span(root.span());
        if dispatch {
            // __locus_ghosts!(root; &place);  resolved in the root's own context
            let mut args = TokenStream::from(TokenTree::Ident(root.clone()));
            args.extend([TokenTree::Punct(Punct::new(';', Spacing::Alone)), TokenTree::Punct(amp)]);
            args.extend(place.iter().cloned());
            body.extend([
                TokenTree::Ident(Ident::new("__locus_ghosts", root.span())),
                TokenTree::Punct(Punct::new('!', Spacing::Alone)),
                TokenTree::Group(Group::new(Delimiter::Parenthesis, args)),
                TokenTree::Punct(Punct::new(';', Spacing::Alone)),
            ]);
            continue;
        }
        // ::__locus::observe(&place);  or  ::__locus::observe(&root);
        let mut arg = TokenStream::from(TokenTree::Punct(amp));
        if ghost {
            arg.extend([place[0].clone()]);
        } else {
            arg.extend(place.iter().cloned());
        }
        body.extend(path("::__locus::observe"));
        body.extend([
            TokenTree::Group(Group::new(Delimiter::Parenthesis, arg)),
            TokenTree::Punct(Punct::new(';', Spacing::Alone)),
        ]);
    }
    body.extend(path(&format!("::__locus::{marker}")));
    TokenStream::from(TokenTree::Group(Group::new(Delimiter::Brace, body)))
}

/// Inline invisible groups, which is how `$x:expr` arrives from macro_rules.
#[cfg(locus_check)]
fn flatten(ts: TokenStream, out: &mut Vec<TokenTree>) {
    for t in ts {
        match t {
            TokenTree::Group(g) if g.delimiter() == Delimiter::None => flatten(g.stream(), out),
            t => out.push(t),
        }
    }
}

#[cfg(locus_check)]
fn is_punct(t: Option<&TokenTree>, c: char) -> bool {
    matches!(t, Some(TokenTree::Punct(p)) if p.as_char() == c)
}

/// A place is a lowercase root followed by `.field`s, stopping before a
/// method call. Calls, macros, paths and keywords are not roots. Enough for
/// the spike's logical code; Locus's real parser replaces this.
#[cfg(locus_check)]
fn find_places(ts: TokenStream, out: &mut Vec<Vec<TokenTree>>) {
    const KEYWORDS: &[&str] = &["true", "false", "forall", "exists", "as", "if", "else", "in"];
    let mut toks = Vec::new();
    flatten(ts, &mut toks);
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            TokenTree::Group(g) => {
                find_places(g.stream(), out);
                i += 1;
            }
            TokenTree::Ident(id) => {
                let name = id.to_string();
                let after_dot = i > 0 && is_punct(toks.get(i - 1), '.');
                let after_colon = i > 0 && is_punct(toks.get(i - 1), ':');
                let next_is_call = matches!(toks.get(i + 1), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis);
                let next_is_path = is_punct(toks.get(i + 1), ':') || is_punct(toks.get(i + 1), '!');
                let lower = name.starts_with(|c: char| c.is_lowercase() || c == '_');
                if after_dot || after_colon || next_is_call || next_is_path || !lower || KEYWORDS.contains(&name.as_str()) {
                    i += 1;
                    continue;
                }
                let mut place = vec![toks[i].clone()];
                let mut j = i + 1;
                while is_punct(toks.get(j), '.')
                    && matches!(toks.get(j + 1), Some(TokenTree::Ident(_) | TokenTree::Literal(_)))
                    && !matches!(toks.get(j + 2), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis)
                {
                    place.push(toks[j].clone());
                    place.push(toks[j + 1].clone());
                    j += 2;
                }
                out.push(place);
                i = j;
            }
            _ => i += 1,
        }
    }
}

/// Option (b): lowering wrote, for each binding of a name that is ever ghost,
/// a line `file name ghost|runtime start_line:start_col end_line:end_col`,
/// with the binding's lexical scope in the lowered file. The innermost scope
/// containing the root's position decides.
#[cfg(locus_check)]
fn table_says_ghost(root: &Ident) -> bool {
    // No table, or none for this file: lowering found no ghost bindings.
    let Ok(path) = std::env::var("LOCUS_SCOPES") else { return false };
    let Ok(text) = std::fs::read_to_string(path) else { return false };
    let s: Span = root.span();
    let file = s.local_file().and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned()));
    let at = (s.line(), s.column());
    let pos = |t: &str| {
        let (l, c) = t.split_once(':').unwrap();
        (l.parse::<usize>().unwrap(), c.parse::<usize>().unwrap())
    };
    let mut best: Option<((usize, usize), bool)> = None;
    for line in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let (start, end) = (pos(f[3]), pos(f[4]));
        if Some(f[0]) == file.as_deref() && f[1] == root.to_string() && start <= at && at <= end && best.is_none_or(|(b, _)| start > b) {
            best = Some((start, f[2] == "ghost"));
        }
    }
    best.is_some_and(|(_, ghost)| ghost)
}
