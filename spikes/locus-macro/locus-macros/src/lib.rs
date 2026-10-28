//! Crate `locus-macros`: the spike's toy `locus!`, and `prove!`, `logic!` and
//! `prop!` for the invocations that rustc expands.
//!
//! `locus!` takes items written in Locus's surface syntax and writes them as
//! one Rust text, following the rules of `spikes/one-text-lowering` (copied,
//! not depended on). The text is the same under both `cfg`s: the library's
//! `proof!`, `data!` and `observe!` put `#[cfg(__locus_check)]` in their own
//! output, so the checking expansion observes and probes, and the runtime
//! expansion gives bare markers. Nothing here reads a `cfg`.
//!
//! This is a token rewriter with heuristics, not Locus's parser. It handles
//! what the spike's examples write and nothing more.
extern crate proc_macro;

mod dump;

use proc_macro2::{
    Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree as TT,
};

// ---------------------------------------------------------------- entry points

/// The toy: lowers the items it wraps.
///
/// Spike-only directives, written first inside the block, select the
/// cross-crate strategies of question 5:
/// - `#![spike(strip_proof_params)]`: a proof parameter exists only under
///   `--cfg __locus_check`, so the runtime signature has no proof parameter.
/// - `#![spike(drop_hole_args)]`: a call whose argument list contains a hole
///   `_` drops that argument from the runtime call.
#[proc_macro]
pub fn locus(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let toks: Vec<TT> = TokenStream::from(input).into_iter().collect();
    let mut cx = Cx::default();
    let start = take_directives(&toks, &mut cx);
    let toks = &toks[start..];
    collect_logical_types(toks, &mut cx);
    let mut out = TokenStream::new();
    for item in split_items(toks) {
        out.extend(lower_item(item, &cx));
    }
    // What lowering's generated module would give through `prelude_import`.
    out.extend(parse(
        "#[allow(unused_imports)] use ::locus::prelude::*;",
        Span::call_site(),
    ));
    dump_expansion(&out);
    out.into()
}

/// With `LOCUS_SPIKE_EXPANSION=dir`, writes each expansion to
/// `dir/<file>.<line>.rs`, for run.sh to show (a spike convenience: rustc
/// does not track the variable).
fn dump_expansion(out: &TokenStream) {
    let Ok(dir) = std::env::var("LOCUS_SPIKE_EXPANSION") else {
        return;
    };
    let at = proc_macro::Span::call_site();
    let file = at
        .local_file()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let _ = std::fs::write(format!("{dir}/{file}.{}.rs", at.line()), out.to_string());
}

/// For an invocation that `locus!` did not rewrite, such as inside `vec![..]`.
#[proc_macro]
pub fn prove(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    free_invocation(input.into(), "Erased").into()
}
#[proc_macro]
pub fn logic(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    free_invocation(input.into(), "LogicalData").into()
}
#[proc_macro]
pub fn prop(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    free_invocation(input.into(), "LogicalData").into()
}

/// Question 1: the tokens rustc's lexer gave this invocation, as text.
/// `__lexdump! { "path"; tokens }` writes them to `path` and expands to
/// nothing; `__lexdump! { tokens }` expands to `pub const DUMP: &str`.
#[proc_macro]
pub fn __lexdump(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let toks = collect(input.into());
    if let (Some(TT::Literal(path)), true) = (toks.first(), punct(toks.get(1), ';')) {
        let path = path.to_string().trim_matches('"').to_string();
        std::fs::write(path, dump::dump(toks[2..].iter().cloned().collect())).expect("write dump");
        return proc_macro::TokenStream::new();
    }
    let text = dump::dump(toks.into_iter().collect());
    let mut out = parse("pub const DUMP: &str =", Span::call_site());
    out.extend([TT::Literal(Literal::string(&text))]);
    out.extend(parse(";", Span::call_site()));
    out.into()
}

// ------------------------------------------------------------------- context

#[derive(Default)]
struct Cx {
    /// `#[derive(Logical)]` types written in this invocation, with their
    /// number of type parameters. Only this invocation: `locus!` cannot see
    /// another one.
    logical_types: Vec<(String, usize)>,
    strip_proof_params: bool,
    drop_hole_args: bool,
}

impl Cx {
    fn logical(&self, name: &str) -> Option<usize> {
        self.logical_types
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, k)| *k)
    }
}

fn take_directives(toks: &[TT], cx: &mut Cx) -> usize {
    let mut i = 0;
    while punct(toks.get(i), '#') && punct(toks.get(i + 1), '!') {
        let Some(g) = group(toks.get(i + 2), Delimiter::Bracket) else {
            break;
        };
        let text = g.stream().to_string();
        if !text.starts_with("spike") {
            break;
        }
        cx.strip_proof_params |= text.contains("strip_proof_params");
        cx.drop_hole_args |= text.contains("drop_hole_args");
        i += 3;
    }
    i
}

fn collect_logical_types(toks: &[TT], cx: &mut Cx) {
    for item in split_items(toks) {
        let (_, derive_logical, i) = take_attrs(item);
        let rest = &item[skip_vis(item, i)..];
        if derive_logical && (ident(rest.first(), "struct") || ident(rest.first(), "enum")) {
            if let Some(name) = ident_name(rest.get(1)) {
                let params = type_params(&rest[2..]).len();
                cx.logical_types.push((name, params));
            }
        }
    }
}

// ------------------------------------------------------------- token helpers

fn parse(s: &str, span: Span) -> TokenStream {
    respan(s.parse().expect("toy: bad snippet"), span)
}

fn respan(ts: TokenStream, span: Span) -> TokenStream {
    ts.into_iter()
        .map(|t| match t {
            TT::Group(g) => {
                let mut n = Group::new(g.delimiter(), respan(g.stream(), span));
                n.set_span(span);
                TT::Group(n)
            }
            mut t => {
                t.set_span(span);
                t
            }
        })
        .collect()
}

fn regroup(g: &Group, stream: TokenStream) -> TT {
    let mut n = Group::new(g.delimiter(), stream);
    n.set_span(g.span());
    TT::Group(n)
}

fn punct(t: Option<&TT>, c: char) -> bool {
    matches!(t, Some(TT::Punct(p)) if p.as_char() == c)
}

fn joint(t: Option<&TT>) -> bool {
    matches!(t, Some(TT::Punct(p)) if p.spacing() == Spacing::Joint)
}

fn ident(t: Option<&TT>, s: &str) -> bool {
    matches!(t, Some(TT::Ident(i)) if i == s)
}

fn ident_name(t: Option<&TT>) -> Option<String> {
    match t {
        Some(TT::Ident(i)) => Some(i.to_string()),
        _ => None,
    }
}

fn group(t: Option<&TT>, d: Delimiter) -> Option<&Group> {
    match t {
        Some(TT::Group(g)) if g.delimiter() == d => Some(g),
        _ => None,
    }
}

fn any_group(t: Option<&TT>) -> Option<&Group> {
    match t {
        Some(TT::Group(g)) => Some(g),
        _ => None,
    }
}

fn collect(ts: TokenStream) -> Vec<TT> {
    ts.into_iter().collect()
}

fn copy(out: &mut TokenStream, toks: &[TT]) {
    out.extend(toks.iter().cloned());
}

const OPS: &str = "=!<>+-*/%^&|";

/// A `:` that is not half of `::`.
fn single_colon(toks: &[TT], i: usize) -> bool {
    punct(toks.get(i), ':')
        && !(joint(toks.get(i)) && punct(toks.get(i + 1), ':'))
        && !(i > 0 && punct(toks.get(i - 1), ':') && joint(toks.get(i - 1)))
}

/// A `=` that is not part of `==`, `=>`, `<=`, `+=`, and so on.
fn single_eq(toks: &[TT], i: usize) -> bool {
    punct(toks.get(i), '=')
        && !(joint(toks.get(i)) && (punct(toks.get(i + 1), '=') || punct(toks.get(i + 1), '>')))
        && !(i > 0
            && joint(toks.get(i - 1))
            && matches!(toks.get(i - 1), Some(TT::Punct(p)) if OPS.contains(p.as_char())))
}

fn fat_arrow(toks: &[TT], i: usize) -> bool {
    punct(toks.get(i), '=') && joint(toks.get(i)) && punct(toks.get(i + 1), '>')
}

fn thin_arrow(toks: &[TT], i: usize) -> bool {
    punct(toks.get(i), '-') && joint(toks.get(i)) && punct(toks.get(i + 1), '>')
}

/// Splits at `sep` outside `<...>`; each part with the separator after it.
fn split_top(toks: &[TT], sep: char) -> Vec<(&[TT], Option<&TT>)> {
    let mut parts = Vec::new();
    let (mut depth, mut start) = (0i32, 0);
    for i in 0..toks.len() {
        if punct(toks.get(i), '<') {
            depth += 1;
        } else if punct(toks.get(i), '>')
            && !(i > 0 && (thin_arrow(toks, i - 1) || fat_arrow(toks, i - 1)))
        {
            depth -= 1;
        } else if depth <= 0 && punct(toks.get(i), sep) {
            parts.push((&toks[start..i], Some(&toks[i])));
            start = i + 1;
        }
    }
    if start < toks.len() {
        parts.push((&toks[start..], None));
    }
    parts
}

fn lowercase(name: &str) -> bool {
    name.starts_with(|c: char| c.is_lowercase() || c == '_')
}

// ----------------------------------------------------------------------- items

/// Item boundaries: a `;`, or the first brace group, except for items that
/// can contain a brace group before their `;`.
fn split_items(toks: &[TT]) -> Vec<&[TT]> {
    let mut items = Vec::new();
    let mut start = 0;
    while start < toks.len() {
        let semi_only = ends_at_semi_only(&toks[start..]);
        let mut end = toks.len();
        for i in start..toks.len() {
            let brace = group(toks.get(i), Delimiter::Brace).is_some();
            if punct(toks.get(i), ';') || (brace && !semi_only) {
                end = i + 1;
                break;
            }
        }
        items.push(&toks[start..end]);
        start = end;
    }
    items
}

fn ends_at_semi_only(item: &[TT]) -> bool {
    let (_, _, i) = take_attrs(item);
    let i = skip_vis(item, i);
    match ident_name(item.get(i)).as_deref() {
        Some("static" | "use" | "type") => true,
        Some("const") => !matches!(
            ident_name(item.get(i + 1)).as_deref(),
            Some("fn" | "unsafe" | "async" | "extern")
        ),
        Some("extern") => ident(item.get(i + 1), "crate"),
        _ => false,
    }
}

/// Outer attributes, minus a `derive(Logical)`; whether there was one; and
/// the index after them.
fn take_attrs(item: &[TT]) -> (TokenStream, bool, usize) {
    let (mut attrs, mut logical, mut i) = (TokenStream::new(), false, 0);
    while punct(item.get(i), '#') {
        let Some(g) = group(item.get(i + 1), Delimiter::Bracket) else {
            break;
        };
        let inner = collect(g.stream());
        let is_derive = ident(inner.first(), "derive");
        if is_derive && g.stream().to_string().contains("Logical") {
            logical = true;
        } else {
            copy(&mut attrs, &item[i..i + 2]);
        }
        i += 2;
    }
    (attrs, logical, i)
}

fn skip_vis(item: &[TT], i: usize) -> usize {
    if !ident(item.get(i), "pub") {
        return i;
    }
    if group(item.get(i + 1), Delimiter::Parenthesis).is_some() {
        i + 2
    } else {
        i + 1
    }
}

/// Visibility, with `pub(locus)` written as `pub`.
fn lower_vis(item: &[TT], i: usize) -> TokenStream {
    let end = skip_vis(item, i);
    let mut out = TokenStream::new();
    match group(item.get(i + 1), Delimiter::Parenthesis) {
        Some(g) if end == i + 2 && g.stream().to_string() == "locus" => {
            copy(&mut out, &item[i..i + 1])
        }
        _ => copy(&mut out, &item[i..end]),
    }
    out
}

fn lower_item(item: &[TT], cx: &Cx) -> TokenStream {
    let (attrs, derive_logical, i) = take_attrs(item);
    let vis = lower_vis(item, i);
    let rest = &item[skip_vis(item, i)..];
    let mut out = TokenStream::new();
    let first = ident_name(rest.first()).unwrap_or_default();
    // `logic fn`: a contextual keyword, so `r#logic` and `fn logic` are not it.
    if first == "logic" && ident(rest.get(1), "fn") {
        return lower_fn(attrs, vis, &rest[1..], cx, true);
    }
    if first == "prop"
        && ident_name(rest.get(1)).is_some()
        && group(rest.get(2), Delimiter::Parenthesis).is_some()
    {
        return lower_prop(vis, rest);
    }
    let kw = rest.iter().take(4).find_map(|t| match t {
        TT::Ident(i)
            if ["fn", "impl", "struct", "enum", "union", "mod", "trait"]
                .contains(&i.to_string().as_str()) =>
        {
            Some(i.to_string())
        }
        _ => None,
    });
    let qualifiers_only = |k: &str| {
        rest.iter().take_while(|t| !ident(Some(t), k)).all(|t| {
            matches!(t, TT::Literal(_))
                || ["const", "unsafe", "async", "extern", "default", "safe"]
                    .iter()
                    .any(|q| ident(Some(t), q))
        })
    };
    match kw.as_deref() {
        Some("fn") if qualifiers_only("fn") => lower_fn(attrs, vis, rest, cx, false),
        Some(k @ ("struct" | "enum")) if derive_logical && qualifiers_only(k) => {
            lower_logical_type(attrs, vis, rest, cx)
        }
        Some(k @ ("struct" | "enum" | "union")) if qualifiers_only(k) => {
            out.extend(attrs);
            out.extend(vis);
            out.extend(lower_adt(rest));
            out
        }
        Some("impl") if qualifiers_only("impl") => {
            out.extend(attrs);
            out.extend(vis);
            out.extend(lower_impl(rest, cx));
            out
        }
        Some("mod")
            if ident(rest.first(), "mod") && group(rest.get(2), Delimiter::Brace).is_some() =>
        {
            let body = group(rest.get(2), Delimiter::Brace).unwrap();
            let inner = collect(body.stream());
            let mut lowered = TokenStream::new();
            for item in split_items(&inner) {
                lowered.extend(lower_item(item, cx));
            }
            lowered.extend(parse(
                "#[allow(unused_imports)] use ::locus::prelude::*;",
                Span::call_site(),
            ));
            out.extend(attrs);
            out.extend(vis);
            copy(&mut out, &rest[..2]);
            out.extend([regroup(body, lowered)]);
            out
        }
        _ => {
            // `use`, `const`, `macro_rules!`, other macros' invocations, traits:
            // copied as written. A macro's input is never rewritten.
            out.extend(attrs);
            out.extend(vis);
            copy(&mut out, rest);
            out
        }
    }
}

/// A runtime `fn`, or a `logic fn` (`logic` already removed), whose body
/// becomes a stub.
fn lower_fn(
    attrs: TokenStream,
    vis: TokenStream,
    toks: &[TT],
    cx: &Cx,
    logic: bool,
) -> TokenStream {
    let fn_at = toks.iter().position(|t| ident(Some(t), "fn")).unwrap();
    let mut j = fn_at + 2;
    let mut depth = 0;
    while j < toks.len() {
        if punct(toks.get(j), '<') {
            depth += 1;
        } else if punct(toks.get(j), '>') {
            depth -= 1;
        } else if depth == 0 && group(toks.get(j), Delimiter::Parenthesis).is_some() {
            break;
        }
        j += 1;
    }
    let params = group(toks.get(j), Delimiter::Parenthesis).expect("toy: fn without parameters");
    let body_at = match toks.last() {
        Some(TT::Group(g)) if g.delimiter() == Delimiter::Brace => toks.len() - 1,
        _ => toks.len(),
    };
    let mut out = TokenStream::new();
    out.extend(attrs);
    if logic {
        out.extend(parse(
            "#[allow(dead_code, non_snake_case, unused_variables)]",
            toks[fn_at].span(),
        ));
    }
    out.extend(vis);
    copy(&mut out, &toks[..j]);
    let mut ghosts = Vec::new();
    out.extend([regroup(
        params,
        lower_params(params, cx, logic, &mut ghosts),
    )]);
    // `-> T where ...`
    let tail = &toks[j + 1..body_at];
    let mut k = 0;
    if thin_arrow(tail, 0) {
        copy(&mut out, &tail[..2]);
        k = 2;
        while k < tail.len() && !ident(tail.get(k), "where") && !punct(tail.get(k), ';') {
            k += 1;
        }
        out.extend(lower_type(&tail[2..k]));
    }
    copy(&mut out, &tail[k..]);
    if let Some(TT::Group(body)) = toks.get(body_at) {
        if logic {
            out.extend([regroup(body, parse("::locus::stub()", body.span()))]);
        } else {
            out.extend([regroup(
                body,
                lower_seq(&collect(body.stream()), cx, &mut ghosts),
            )]);
        }
    }
    out
}

fn lower_params(params: &Group, cx: &Cx, logic: bool, ghosts: &mut Vec<String>) -> TokenStream {
    let toks = collect(params.stream());
    let mut out = TokenStream::new();
    for (part, sep) in split_top(&toks, ',') {
        let (attrs, _, a) = take_attrs(part);
        match (a..part.len()).find(|&c| single_colon(part, c)) {
            Some(c) => {
                let ty = &part[c + 1..];
                let proof = punct(ty.first(), '@');
                out.extend(attrs);
                if proof && !logic {
                    // A ghost parameter: lowering marks it for the runtime lints.
                    out.extend(parse("#[allow(unused_variables)]", part[c].span()));
                    if cx.strip_proof_params {
                        out.extend(parse("#[cfg(__locus_check)]", part[c].span()));
                    }
                }
                if proof || is_logical_type(ty, cx) {
                    ghosts.extend(binding_names(&part[a..c]));
                }
                copy(&mut out, &part[a..=c]);
                out.extend(lower_type(ty));
            }
            None => copy(&mut out, part),
        }
        if let Some(s) = sep {
            out.extend([s.clone()]);
        }
    }
    out
}

fn is_logical_type(ty: &[TT], cx: &Cx) -> bool {
    match ident_name(ty.first()) {
        Some(n) => ["Nat", "Int", "Prop", "Seq"].contains(&n.as_str()) || cx.logical(&n).is_some(),
        None => false,
    }
}

fn binding_names(pat: &[TT]) -> Vec<String> {
    pat.iter()
        .filter_map(|t| ident_name(Some(t)))
        .filter(|n| n != "mut" && n != "ref" && n != "_" && lowercase(n))
        .collect()
}

/// A type: `@P` becomes `::locus::Proof![P]`, and named tuple components
/// lose their names.
fn lower_type(toks: &[TT]) -> TokenStream {
    let mut out = TokenStream::new();
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            TT::Punct(p) if p.as_char() == '@' => {
                let span = p.span();
                let prop: Vec<TT> = if let Some(g) = group(toks.get(i + 1), Delimiter::Parenthesis)
                {
                    i += 2;
                    collect(g.stream())
                } else {
                    // `@Even(4)`, `@False`: a path, and its arguments.
                    let mut j = i + 1;
                    while ident_name(toks.get(j)).is_some() {
                        j += 1;
                        if punct(toks.get(j), ':') && punct(toks.get(j + 1), ':') {
                            j += 2;
                        } else {
                            break;
                        }
                    }
                    if group(toks.get(j), Delimiter::Parenthesis).is_some() {
                        j += 1;
                    }
                    let prop = toks[i + 1..j].to_vec();
                    i = j;
                    prop
                };
                out.extend(parse("::locus::Proof!", span));
                let mut g = Group::new(Delimiter::Bracket, prop.into_iter().collect());
                g.set_span(span);
                out.extend([TT::Group(g)]);
            }
            TT::Group(g) => {
                let inner = collect(g.stream());
                let inner = if g.delimiter() == Delimiter::Parenthesis {
                    strip_names(&inner)
                } else {
                    inner
                };
                out.extend([regroup(g, lower_type(&inner))]);
                i += 1;
            }
            t => {
                out.extend([t.clone()]);
                i += 1;
            }
        }
    }
    out
}

/// `(out: T, @P)` in a type is `(T, @P)`.
fn strip_names(toks: &[TT]) -> Vec<TT> {
    let mut out = Vec::new();
    for (part, sep) in split_top(toks, ',') {
        let named = ident_name(part.first()).is_some() && single_colon(part, 1);
        out.extend(part[if named { 2 } else { 0 }..].iter().cloned());
        out.extend(sep.cloned());
    }
    out
}

/// A struct, enum or union that is not logical: proof fields get the marker.
fn lower_adt(toks: &[TT]) -> TokenStream {
    let mut out = TokenStream::new();
    for (i, t) in toks.iter().enumerate() {
        match t {
            TT::Group(g) if g.delimiter() == Delimiter::Brace && ident(toks.first(), "struct") => {
                out.extend([regroup(g, lower_fields(&collect(g.stream()), true))]);
            }
            TT::Group(g)
                if g.delimiter() == Delimiter::Parenthesis
                    && ident(toks.first(), "struct")
                    && i > 1 =>
            {
                out.extend([regroup(g, lower_fields(&collect(g.stream()), false))]);
            }
            TT::Group(g) if g.delimiter() == Delimiter::Brace => {
                // enum variants
                out.extend([regroup(g, lower_type(&collect(g.stream())))]);
            }
            t => out.extend([t.clone()]),
        }
    }
    out
}

fn lower_fields(toks: &[TT], named: bool) -> TokenStream {
    let mut out = TokenStream::new();
    for (part, sep) in split_top(toks, ',') {
        let (attrs, _, a) = take_attrs(part);
        out.extend(attrs);
        if named {
            // `pub(locus) x: T` is a visibility; in a tuple struct, `pub (locus)`
            // keeps its Rust meaning (a public field of type `(locus)`).
            let v = skip_vis(part, a);
            let ty_at = (v..part.len())
                .find(|&c| single_colon(part, c))
                .map_or(part.len(), |c| c + 1);
            if punct(part.get(ty_at), '@') {
                out.extend(parse("#[allow(dead_code)]", part[ty_at].span()));
            }
            out.extend(lower_vis(part, a));
            copy(&mut out, &part[v..ty_at]);
            out.extend(lower_type(&part[ty_at..]));
        } else {
            if part.iter().any(|t| punct(Some(t), '@')) {
                out.extend(parse("#[allow(dead_code)]", part[a].span()));
            }
            out.extend(lower_type(&part[a..]));
        }
        out.extend(sep.cloned());
    }
    out
}

/// Type parameter names of `<T, U>` at the start of `toks`.
fn type_params(toks: &[TT]) -> Vec<String> {
    if !punct(toks.first(), '<') {
        return Vec::new();
    }
    let mut names = Vec::new();
    let mut depth = 0;
    for (i, t) in toks.iter().enumerate() {
        if punct(Some(t), '<') {
            depth += 1;
        } else if punct(Some(t), '>') {
            depth -= 1;
            if depth == 0 {
                break;
            }
        } else if depth == 1 && (i == 1 || punct(toks.get(i - 1), ',')) {
            if let Some(n) = ident_name(Some(t)) {
                names.push(n);
            }
        }
    }
    names
}

/// `#[derive(Logical)]`: a stub struct, its projection, an alias, and a stub
/// function for each variant.
fn lower_logical_type(attrs: TokenStream, vis: TokenStream, toks: &[TT], cx: &Cx) -> TokenStream {
    let is_enum = ident(toks.first(), "enum");
    let name = ident_name(toks.get(1)).unwrap();
    let span = toks[1].span();
    let params = type_params(&toks[2..]);
    let gp = if params.is_empty() {
        String::new()
    } else {
        format!("<{}>", params.join(", "))
    };
    let vis_s = vis.to_string();
    let field = if params.is_empty() {
        String::new()
    } else {
        format!("(::std::marker::PhantomData<({},)>)", params.join(", "))
    };
    let mut s = format!(
        "#[allow(non_camel_case_types, dead_code)] {vis_s} struct __locus_{name}{gp}{field};
         impl{gp} ::locus::Proj for __locus_{name}{gp} {{ type Out = ::locus::LogicalData; }}
         #[allow(dead_code)] {vis_s} type {name}{gp} = <__locus_{name}{gp} as ::locus::Proj>::Out;"
    );
    if is_enum {
        let body = toks
            .last()
            .and_then(|t| group(Some(t), Delimiter::Brace))
            .unwrap();
        let body = collect(body.stream());
        let mut fns = String::new();
        for (part, _) in split_top(&body, ',') {
            let Some(variant) = ident_name(part.first()) else {
                continue;
            };
            let args = match group(part.get(1), Delimiter::Parenthesis) {
                Some(g) => split_top(&collect(g.stream()), ',')
                    .into_iter()
                    .map(|(ty, _)| format!("_: {}", lower_type(ty)))
                    .collect::<Vec<_>>()
                    .join(", "),
                None => String::new(),
            };
            fns.push_str(&format!(
                "{vis_s} fn {variant}({args}) -> {name}{gp} {{ ::locus::stub() }}\n"
            ));
        }
        s.push_str(&format!(
            "#[allow(non_snake_case, dead_code)] impl{gp} __locus_{name}{gp} {{ {fns} }}"
        ));
    }
    let _ = cx;
    let mut out = attrs;
    out.extend(parse(&s, span));
    out
}

/// `prop Name(params) { Arm(fields) => { .. } .. }`: a stub function for the
/// proposition, and a field-less struct of the same name, in the type
/// namespace only, whose stub functions are the named arms.
fn lower_prop(vis: TokenStream, toks: &[TT]) -> TokenStream {
    let name = ident_name(toks.get(1)).unwrap();
    let span = toks[1].span();
    let params = group(toks.get(2), Delimiter::Parenthesis).unwrap();
    let mut ghosts = Vec::new();
    let cx = Cx::default();
    let lowered_params = lower_params(params, &cx, true, &mut ghosts);
    let vis_s = vis.to_string();
    let mut arms = String::new();
    if let Some(body) = group(toks.get(3), Delimiter::Brace) {
        let body = collect(body.stream());
        let mut i = 0;
        while i < body.len() {
            if let Some(arm) = ident_name(body.get(i)) {
                let args = match group(body.get(i + 1), Delimiter::Parenthesis) {
                    Some(g) => lower_params(g, &cx, true, &mut ghosts).to_string(),
                    None => String::new(),
                };
                arms.push_str(&format!(
                    "{vis_s} fn {arm}({args}) -> ::locus::Erased {{ ::locus::stub() }}\n"
                ));
                // skip to the arm's body
                while i < body.len() && group(body.get(i), Delimiter::Brace).is_none() {
                    i += 1;
                }
            }
            i += 1;
        }
    }
    let s = format!(
        "#[allow(non_snake_case, dead_code, unused_variables)] {vis_s} fn {name}({lowered_params}) -> ::locus::Prop {{ ::locus::stub() }}
         #[allow(non_camel_case_types, dead_code)] {vis_s} struct {name} {{}}
         #[allow(non_snake_case, dead_code, unused_variables)] impl {name} {{ {arms} }}"
    );
    parse(&s, span)
}

/// An `impl`: the self type is renamed when it is a logical type of this
/// invocation, and its items are lowered.
fn lower_impl(toks: &[TT], cx: &Cx) -> TokenStream {
    let mut out = TokenStream::new();
    let Some(TT::Group(body)) = toks.last() else {
        copy(&mut out, toks);
        return out;
    };
    let header = &toks[..toks.len() - 1];
    let self_from = header
        .iter()
        .position(|t| ident(Some(t), "for"))
        .unwrap_or(0);
    for (i, t) in header.iter().enumerate() {
        match t {
            TT::Ident(id)
                if i > self_from
                    && cx.logical(&id.to_string()).is_some()
                    && !punct(header.get(i.wrapping_sub(1)), ':') =>
            {
                out.extend([TT::Ident(Ident::new(&format!("__locus_{id}"), id.span()))]);
            }
            t => out.extend([t.clone()]),
        }
    }
    let inner = collect(body.stream());
    let mut lowered = TokenStream::new();
    for item in split_items(&inner) {
        lowered.extend(lower_item(item, cx));
    }
    out.extend([regroup(body, lowered)]);
    out
}

// ---------------------------------------------------------------------- bodies

/// A proof hole: `_` in an expression.
fn hole(span: Span) -> TokenStream {
    parse("::locus::proof!([] [])", span)
}

fn closure_start(prev: Option<&TT>) -> bool {
    match prev {
        None => true,
        Some(TT::Punct(p)) => p.as_char() != '?',
        Some(TT::Ident(i)) => i == "move" || i == "return",
        _ => false,
    }
}

/// Statements and expressions of a runtime body.
fn lower_seq(toks: &[TT], cx: &Cx, ghosts: &mut Vec<String>) -> TokenStream {
    let mut out = TokenStream::new();
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        let prev = if i > 0 { toks.get(i - 1) } else { None };
        if cx.drop_hole_args {
            if let Some(next) = call_with_hole(toks, i, prev, cx, ghosts, &mut out) {
                i = next;
                continue;
            }
        }
        match t {
            TT::Ident(id) => {
                let s = id.to_string();
                let condition = ident(prev, "if") || ident(prev, "while") || punct(prev, '&');
                match s.as_str() {
                    "let" if condition => {
                        // `if let PAT =`: the pattern is Rust's
                        while i < toks.len() && !single_eq(toks, i) {
                            out.extend([toks[i].clone()]);
                            i += 1;
                        }
                        continue;
                    }
                    "let" => {
                        let end = (i..toks.len())
                            .find(|&k| punct(toks.get(k), ';'))
                            .unwrap_or(toks.len());
                        out.extend(lower_let(&toks[i..end], toks.get(end), cx, ghosts));
                        i = end + 1;
                        continue;
                    }
                    "match" => {
                        out.extend([t.clone()]);
                        let j = (i + 1..toks.len())
                            .find(|&k| group(toks.get(k), Delimiter::Brace).is_some())
                            .unwrap_or(toks.len());
                        out.extend(lower_seq(&toks[i + 1..j], cx, ghosts));
                        if let Some(TT::Group(arms)) = toks.get(j) {
                            out.extend([regroup(
                                arms,
                                lower_arms(&collect(arms.stream()), cx, ghosts),
                            )]);
                        }
                        i = j + 1;
                        continue;
                    }
                    "for" => {
                        while i < toks.len() && !ident(toks.get(i), "in") {
                            out.extend([toks[i].clone()]);
                            i += 1;
                        }
                        continue;
                    }
                    "as" => {
                        copy(&mut out, &toks[i..(i + 2).min(toks.len())]);
                        i += 2;
                        continue;
                    }
                    "_" if !single_eq(toks, i + 1) => {
                        out.extend(hole(id.span()));
                        i += 1;
                        continue;
                    }
                    "prove" | "logic" | "prop"
                        if punct(toks.get(i + 1), '!') && any_group(toks.get(i + 2)).is_some() =>
                    {
                        let kind = if s == "prove" { "proof" } else { "data" };
                        out.extend(lower_logic(
                            kind,
                            any_group(toks.get(i + 2)).unwrap(),
                            id.span(),
                            cx,
                            ghosts,
                        ));
                        i += 3;
                        continue;
                    }
                    _ if punct(toks.get(i + 1), '!') && !punct(toks.get(i + 2), '=') => {
                        // Another macro: its input is left alone.
                        copy(&mut out, &toks[i..i + 2]);
                        i += 2;
                        if ident_name(toks.get(i)).is_some() && any_group(toks.get(i + 1)).is_some()
                        {
                            copy(&mut out, &toks[i..i + 2]);
                            i += 2;
                        } else if any_group(toks.get(i)).is_some() {
                            copy(&mut out, &toks[i..i + 1]);
                            i += 1;
                        }
                        continue;
                    }
                    _ => {}
                }
            }
            TT::Punct(p) if p.as_char() == '|' && closure_start(prev) => {
                // Closure parameters are patterns.
                out.extend([t.clone()]);
                if joint(Some(t)) && punct(toks.get(i + 1), '|') {
                    out.extend([toks[i + 1].clone()]);
                    i += 2;
                    continue;
                }
                i += 1;
                while i < toks.len() && !punct(toks.get(i), '|') {
                    out.extend([toks[i].clone()]);
                    i += 1;
                }
                if i < toks.len() {
                    out.extend([toks[i].clone()]);
                    i += 1;
                }
                continue;
            }
            TT::Punct(p)
                if p.as_char() == ':'
                    && joint(Some(t))
                    && punct(toks.get(i + 1), ':')
                    && punct(toks.get(i + 2), '<') =>
            {
                // A turbofish holds types: `Vec::<_>` keeps its `_`.
                copy(&mut out, &toks[i..i + 2]);
                i += 2;
                let start = i;
                let mut depth = 0;
                while i < toks.len() {
                    if punct(toks.get(i), '<') {
                        depth += 1;
                    } else if punct(toks.get(i), '>') && !thin_arrow(toks, i - 1) {
                        depth -= 1;
                    }
                    i += 1;
                    if depth == 0 {
                        break;
                    }
                }
                out.extend(lower_type(&toks[start..i]));
                continue;
            }
            TT::Group(g) => {
                let inner = collect(g.stream());
                let lowered = if g.delimiter() != Delimiter::Brace && single_eq(toks, i + 1) {
                    // `(a, _) = pair;`: the left side of an assignment is a pattern.
                    g.stream()
                } else if g.delimiter() == Delimiter::Bracket {
                    // `[x; N]`: the length is a constant, and `_` there is Rust's.
                    match inner.iter().position(|t| punct(Some(t), ';')) {
                        Some(k) => {
                            let mut s = lower_seq(&inner[..k], cx, ghosts);
                            copy(&mut s, &inner[k..]);
                            s
                        }
                        None => lower_seq(&inner, cx, ghosts),
                    }
                } else if g.delimiter() == Delimiter::Brace {
                    let mut scope = ghosts.clone();
                    lower_seq(&inner, cx, &mut scope)
                } else {
                    lower_seq(&inner, cx, ghosts)
                };
                out.extend([regroup(g, lowered)]);
                i += 1;
                continue;
            }
            _ => {}
        }
        out.extend([t.clone()]);
        i += 1;
    }
    out
}

fn lower_arms(toks: &[TT], cx: &Cx, ghosts: &mut Vec<String>) -> TokenStream {
    let mut out = TokenStream::new();
    let mut i = 0;
    while i < toks.len() {
        let start = i;
        while i < toks.len() && !fat_arrow(toks, i) {
            i += 1;
        }
        let pat = &toks[start..i];
        match pat.iter().position(|t| ident(Some(t), "if")) {
            Some(k) => {
                copy(&mut out, &pat[..k]);
                out.extend(lower_seq(&pat[k..], cx, ghosts));
            }
            None => copy(&mut out, pat),
        }
        if i >= toks.len() {
            break;
        }
        copy(&mut out, &toks[i..i + 2]);
        i += 2;
        if group(toks.get(i), Delimiter::Brace).is_some() {
            out.extend(lower_seq(&toks[i..i + 1], cx, ghosts));
            i += 1;
            if punct(toks.get(i), ',') {
                out.extend([toks[i].clone()]);
                i += 1;
            }
        } else {
            let s = i;
            while i < toks.len() && !punct(toks.get(i), ',') {
                i += 1;
            }
            out.extend(lower_seq(&toks[s..i], cx, ghosts));
            if i < toks.len() {
                out.extend([toks[i].clone()]);
                i += 1;
            }
        }
    }
    out
}

/// `let PAT: TYPE = INIT;` (without the `;`, given separately).
fn lower_let(toks: &[TT], semi: Option<&TT>, cx: &Cx, ghosts: &mut Vec<String>) -> TokenStream {
    let mut out = TokenStream::new();
    let mut i = 1;
    while i < toks.len() && !single_colon(toks, i) && !single_eq(toks, i) {
        i += 1;
    }
    let pat = &toks[1..i];
    let pat_end = i;
    let mut ty: &[TT] = &[];
    if single_colon(toks, i) {
        i += 1;
        let s = i;
        while i < toks.len() && !single_eq(toks, i) {
            i += 1;
        }
        ty = &toks[s..i];
    }
    let init = if single_eq(toks, i) {
        &toks[i + 1..]
    } else {
        &[]
    };
    let locus_macro = init.len() == 3
        && punct(init.get(1), '!')
        && matches!(
            ident_name(init.first()).as_deref(),
            Some("prove" | "logic" | "prop")
        );
    let is_hole = init.len() == 1 && ident(init.first(), "_");
    // A binding shown to be ghost, from the syntax alone.
    let ghost = punct(ty.first(), '@') || is_logical_type(ty, cx) || locus_macro || is_hole;
    if ghost {
        // Lowering marks ghost bindings for the runtime lints.
        out.extend(parse("#[allow(unused_variables)]", toks[0].span()));
    }
    copy(&mut out, &toks[..pat_end]);
    if !ty.is_empty() {
        out.extend([toks[pat_end].clone()]);
        out.extend(lower_type(ty));
    }
    if single_eq(toks, i) {
        out.extend([toks[i].clone()]);
        if locus_macro {
            // Which marker a `logic!` gives depends on what its logical code
            // is. The toy reads it from the annotation: a proof if the `let`
            // says `@P`, logical data otherwise.
            let kind = match ident_name(init.first()).as_deref() {
                Some("prove") => "proof",
                Some("logic") if punct(ty.first(), '@') => "proof",
                _ => "data",
            };
            out.extend(lower_logic(
                kind,
                any_group(init.get(2)).unwrap(),
                init[0].span(),
                cx,
                ghosts,
            ));
        } else if is_hole {
            out.extend(hole(init[0].span()));
        } else {
            out.extend(lower_seq(init, cx, ghosts));
        }
    }
    out.extend(semi.cloned());
    // A proposition in the annotation is read after the initializer.
    let mut prop = Vec::new();
    collect_props(ty, &mut prop);
    if !prop.is_empty() {
        // `i` in `Vec<(i: usize, @(i < n))>` is bound by the type, not a place.
        let mut bound = component_names(ty);
        let mut places = Vec::new();
        find_places(prop.into_iter().collect(), &mut bound, &mut places);
        let span = toks[0].span();
        out.extend(parse("::locus::observe!", span));
        let mut g = Group::new(Delimiter::Parenthesis, borrows(&places, ghosts));
        g.set_span(span);
        out.extend([TT::Group(g)]);
        out.extend(parse(";", span));
    }
    if ghost {
        ghosts.extend(binding_names(pat));
    }
    out
}

/// Names of named tuple components anywhere in a type.
fn component_names(ty: &[TT]) -> Vec<String> {
    let mut names = Vec::new();
    for t in ty {
        if let TT::Group(g) = t {
            let inner = collect(g.stream());
            if g.delimiter() == Delimiter::Parenthesis {
                for (part, _) in split_top(&inner, ',') {
                    if single_colon(part, 1) {
                        names.extend(ident_name(part.first()));
                    }
                }
            }
            names.extend(component_names(&inner));
        }
    }
    names
}

/// The tokens of every proposition `@...` in a type.
fn collect_props(ty: &[TT], out: &mut Vec<TT>) {
    for (i, t) in ty.iter().enumerate() {
        match t {
            TT::Punct(p) if p.as_char() == '@' => {
                if let Some(g) = any_group(ty.get(i + 1)) {
                    out.push(TT::Group(g.clone()));
                } else {
                    out.extend(
                        ty[i + 1..]
                            .iter()
                            .take_while(|t| !punct(Some(t), ','))
                            .cloned(),
                    );
                }
            }
            TT::Group(g) if !punct(ty.get(i.wrapping_sub(1)), '@') => {
                collect_props(&collect(g.stream()), out)
            }
            _ => {}
        }
    }
}

/// `&place, ...`: a ghost root is observed whole (its marker has no fields).
/// The `&` takes the root's span, so a borrow error points at the place.
fn borrows(places: &[Vec<TT>], ghosts: &[String]) -> TokenStream {
    let mut out = TokenStream::new();
    let mut seen = Vec::new();
    for place in places {
        let root = &place[0];
        let ghost = ghosts.contains(&root.to_string());
        let path: Vec<TT> = if ghost {
            vec![root.clone()]
        } else {
            place.clone()
        };
        let key = path.iter().map(|t| t.to_string()).collect::<String>();
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        if !out.is_empty() {
            out.extend([TT::Punct(Punct::new(',', Spacing::Alone))]);
        }
        let mut amp = Punct::new('&', Spacing::Alone);
        amp.set_span(root.span());
        out.extend([TT::Punct(amp)]);
        out.extend(path);
    }
    out
}

/// `prove!(..)`, `logic! {..}`, `prop!(..)` written in a body that `locus!`
/// wraps: `::locus::proof!([&places] [items] code)` or `::locus::data!(..)`.
fn lower_logic(kind: &str, g: &Group, span: Span, cx: &Cx, ghosts: &[String]) -> TokenStream {
    let mut bound = Vec::new();
    let mut places = Vec::new();
    find_places(g.stream(), &mut bound, &mut places);
    let mut items = TokenStream::new();
    for (k, item) in find_items(g.stream(), cx, &bound).into_iter().enumerate() {
        if k > 0 {
            items.extend([TT::Punct(Punct::new(',', Spacing::Alone))]);
        }
        items.extend(item);
    }
    let mut args = TokenStream::new();
    let mut p = Group::new(Delimiter::Bracket, borrows(&places, ghosts));
    p.set_span(span);
    let mut n = Group::new(Delimiter::Bracket, items);
    n.set_span(span);
    args.extend([TT::Group(p), TT::Group(n)]);
    args.extend(g.stream());
    let mut out = parse(&format!("::locus::{kind}!"), span);
    let mut a = Group::new(Delimiter::Parenthesis, args);
    a.set_span(span);
    out.extend([TT::Group(a)]);
    out
}

/// Inline invisible groups, which is how `$x:expr` arrives from macro_rules.
fn flatten(ts: TokenStream, out: &mut Vec<TT>) {
    for t in ts {
        match t {
            TT::Group(g) if g.delimiter() == Delimiter::None => flatten(g.stream(), out),
            t => out.push(t),
        }
    }
}

/// A place is a lowercase root followed by `.field`s, stopping before a
/// method call. Calls, macros, paths, keywords and the binders of logical
/// closures (`|n: Int|`) are not roots. From the earlier spike's toy finder.
fn find_places(ts: TokenStream, bound: &mut Vec<String>, out: &mut Vec<Vec<TT>>) {
    const KEYWORDS: &[&str] = &[
        "true", "false", "forall", "exists", "as", "if", "else", "in", "match", "let", "mut",
        "ref", "move",
    ];
    let mut toks = Vec::new();
    flatten(ts, &mut toks);
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            TT::Group(g) => {
                find_places(g.stream(), bound, out);
                i += 1;
            }
            TT::Punct(p) if p.as_char() == '|' => {
                i += 1;
                while i < toks.len() && !punct(toks.get(i), '|') {
                    let after_colon = i > 0 && punct(toks.get(i - 1), ':');
                    if let (Some(n), false) = (ident_name(toks.get(i)), after_colon) {
                        bound.push(n);
                    }
                    i += 1;
                }
                i += 1;
            }
            TT::Ident(id) => {
                let name = id.to_string();
                let after_dot = i > 0 && punct(toks.get(i - 1), '.');
                let after_colon = i > 0 && punct(toks.get(i - 1), ':');
                let next_is_call = group(toks.get(i + 1), Delimiter::Parenthesis).is_some();
                let next_is_path = punct(toks.get(i + 1), ':') || punct(toks.get(i + 1), '!');
                if after_dot
                    || after_colon
                    || next_is_call
                    || next_is_path
                    || !lowercase(&name)
                    || KEYWORDS.contains(&name.as_str())
                    || bound.contains(&name)
                {
                    i += 1;
                    continue;
                }
                let mut place = vec![toks[i].clone()];
                let mut j = i + 1;
                while punct(toks.get(j), '.')
                    && matches!(toks.get(j + 1), Some(TT::Ident(_) | TT::Literal(_)))
                    && group(toks.get(j + 2), Delimiter::Parenthesis).is_none()
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

/// Names that logical code calls, to be probed: `double`, or `Point::origin`,
/// which is `__locus_Point::origin` when `Point` is a logical type of this
/// invocation, and is written as it stands otherwise.
fn find_items(ts: TokenStream, cx: &Cx, bound: &[String]) -> Vec<TokenStream> {
    let mut toks = Vec::new();
    flatten(ts, &mut toks);
    let mut items = Vec::new();
    for k in 0..toks.len() {
        if let TT::Group(g) = &toks[k] {
            items.extend(find_items(g.stream(), cx, bound));
            continue;
        }
        let Some(name) = ident_name(toks.get(k)) else {
            continue;
        };
        if group(toks.get(k + 1), Delimiter::Parenthesis).is_none()
            || (k > 0 && punct(toks.get(k - 1), '.'))
        {
            continue;
        }
        let mut start = k;
        while start >= 3
            && punct(toks.get(start - 1), ':')
            && punct(toks.get(start - 2), ':')
            && ident_name(toks.get(start - 3)).is_some()
        {
            start -= 3;
        }
        if start == k {
            if lowercase(&name)
                && !bound.contains(&name)
                && !["snap", "final"].contains(&name.as_str())
            {
                items.push(TokenStream::from(toks[k].clone()));
            }
            continue;
        }
        let first = toks[start].to_string();
        let mut item = TokenStream::new();
        match cx.logical(&first) {
            Some(n) => {
                let args = if n == 0 {
                    String::new()
                } else {
                    format!("::<{}>", vec!["::locus::LogicalData"; n].join(", "))
                };
                item.extend(parse(&format!("__locus_{first}{args}"), toks[start].span()));
                copy(&mut item, &toks[start + 1..=k]);
            }
            None => copy(&mut item, &toks[start..=k]),
        }
        items.push(item);
    }
    items
}

/// Question 5's `drop_hole_args`: `f(a, _)` is `f(a, marker)` when checking
/// and `f(a)` at run time. It knows nothing about `f`.
fn call_with_hole(
    toks: &[TT],
    i: usize,
    prev: Option<&TT>,
    cx: &Cx,
    ghosts: &mut Vec<String>,
    out: &mut TokenStream,
) -> Option<usize> {
    if ident_name(toks.get(i)).is_none() || punct(prev, '.') || punct(prev, ':') {
        return None;
    }
    let mut j = i;
    while punct(toks.get(j + 1), ':')
        && punct(toks.get(j + 2), ':')
        && ident_name(toks.get(j + 3)).is_some()
    {
        j += 3;
    }
    let args = group(toks.get(j + 1), Delimiter::Parenthesis)?;
    let inner = collect(args.stream());
    let parts = split_top(&inner, ',');
    let is_hole = |p: &[TT]| p.len() == 1 && ident(p.first(), "_");
    if !parts.iter().any(|(p, _)| is_hole(p)) {
        return None;
    }
    let path = &toks[i..=j];
    let span = toks[i].span();
    let checked = regroup(args, lower_seq(&inner, cx, ghosts));
    let mut kept = Vec::new();
    for (p, sep) in parts.iter().filter(|(p, _)| !is_hole(p)) {
        kept.extend(p.iter().cloned());
        kept.extend(sep.cloned());
    }
    let runtime = regroup(args, lower_seq(&kept, cx, ghosts));
    let mut body = parse("#[cfg(__locus_check)] let __locus_r =", span);
    copy(&mut body, path);
    body.extend([checked]);
    body.extend(parse("; #[cfg(not(__locus_check))] let __locus_r =", span));
    copy(&mut body, path);
    body.extend([runtime]);
    body.extend(parse("; __locus_r", span));
    let mut g = Group::new(Delimiter::Brace, body);
    g.set_span(span);
    out.extend([TT::Group(g)]);
    Some(j + 2)
}

/// `prove!` and friends expanded by rustc: observe each place precisely
/// (the toy has no table of ghost roots), then give the marker.
fn free_invocation(input: TokenStream, marker: &str) -> TokenStream {
    let mut bound = Vec::new();
    let mut places = Vec::new();
    find_places(input, &mut bound, &mut places);
    let mut observe = TokenStream::new();
    for place in &places {
        let mut amp = Punct::new('&', Spacing::Alone);
        amp.set_span(place[0].span());
        let mut arg = TokenStream::from(TT::Punct(amp));
        arg.extend(place.iter().cloned());
        observe.extend(parse("::locus::observe", Span::call_site()));
        observe.extend([TT::Group(Group::new(Delimiter::Parenthesis, arg))]);
        observe.extend(parse(";", Span::call_site()));
    }
    let mut body = parse("#[cfg(__locus_check)]", Span::call_site());
    body.extend([TT::Group(Group::new(Delimiter::Brace, observe))]);
    body.extend(parse(&format!("::locus::{marker}"), Span::call_site()));
    TokenStream::from(TT::Group(Group::new(Delimiter::Brace, body)))
}
