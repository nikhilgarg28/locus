//! Crate `locus-macros`: the spike's toy `locus!`, and `prove!`, `logic!` and
//! `prop!` for the invocations that rustc expands. Adapted (copied, not
//! depended on) from `spikes/locus-macro/locus-macros/src/lib.rs`.
//!
//! What changed from that macro:
//! - a `#[derive(Logical)]` type is a zero-sized struct of its own, under its
//!   own name, where it was an alias of the one marker `LogicalData`;
//! - a ghost local that only logical code mentions holds `::locus::Untyped`,
//!   and any other `logic! { .. }` gives a marker of the type rustc infers;
//! - a trait may declare `logic fn` members, and an `impl` gives them;
//! - a name that logical code calls is probed by `use name as _;`, because a
//!   generic function used as a value needs type arguments, and there is no
//!   longer one marker to write for them.
//!
//! `locus!` writes one Rust text, the same under both `cfg`s: the library's
//! macros put `#[cfg(__locus_check)]` in their own output. Nothing here
//! reads a `cfg`.
//!
//! This is a token rewriter with heuristics, not Locus's parser. Every place
//! where it guesses is marked `GUESS`.
extern crate proc_macro;

use proc_macro2::{Delimiter, Group, Ident, Punct, Spacing, Span, TokenStream, TokenTree as TT};

// ---------------------------------------------------------------- entry points

/// The toy: lowers the items it wraps.
///
/// Spike-only directives, written first inside the block:
/// - `#![spike(logic_members_in_trait)]`: a trait's logic functions stay
///   items of the lowered trait, under their mangled names, where the default
///   puts them in a hidden trait beside it.
/// - `#![spike(allow_ghost_headers)]`: an `impl` whose header writes a proof
///   type or a snapshot type is not rejected, to show what rustc does with it.
///
/// And two environment variables. `LOCUS_SPIKE_ALL_TYPED=1` makes every
/// `let x = logic! { .. };` a typed binding, as if no ghost local could be
/// left without a logical type, for a count. `LOCUS_SPIKE_IN_TRAIT=1` is
/// `logic_members_in_trait` for every invocation, the library's included.
#[proc_macro]
pub fn locus(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let toks: Vec<TT> = TokenStream::from(input).into_iter().collect();
    let mut cx = Cx {
        all_typed: std::env::var("LOCUS_SPIKE_ALL_TYPED").is_ok(),
        members_in_trait: std::env::var("LOCUS_SPIKE_IN_TRAIT").is_ok(),
        ..Cx::default()
    };
    let start = take_directives(&toks, &mut cx);
    let toks = &toks[start..];
    collect_items(toks, &mut cx);
    let mut out = TokenStream::new();
    for item in split_items(toks) {
        out.extend(lower_item(item, &cx, &Place::default()));
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
    free_invocation(input.into(), "::locus::Erased").into()
}
#[proc_macro]
pub fn logic(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    free_invocation(input.into(), "::locus::ghost()").into()
}
#[proc_macro]
pub fn prop(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    free_invocation(input.into(), "::locus::Prop").into()
}

// ------------------------------------------------------------------- context

/// The logical types of Locus's library, with their numbers of parameters.
const LIBRARY_LOGICAL: &[(&str, usize)] =
    &[("Int", 0), ("Nat", 0), ("Bool", 0), ("Prop", 0), ("Seq", 1)];

#[derive(Default)]
struct Cx {
    /// `#[derive(Logical)]` types written in this invocation, with their
    /// number of type parameters. Only this invocation: `locus!` cannot see
    /// another one.
    logical_types: Vec<(String, usize)>,
    /// Traits declared in this invocation.
    traits: Vec<TraitInfo>,
    members_in_trait: bool,
    allow_ghost_headers: bool,
    all_typed: bool,
}

#[derive(Default)]
struct TraitInfo {
    name: String,
    /// It declares a `logic fn`.
    has_logic: bool,
    /// It declares a method that runs.
    runs: bool,
    /// The names in its list of supertraits.
    supers: Vec<String>,
}

impl Cx {
    /// The number of type parameters of a logical type, if `name` is known
    /// to be one. GUESS: by its name alone. A logical type of another
    /// invocation or crate is not known, and a program's own type called
    /// `Nat` would be taken for the library's.
    fn logical(&self, name: &str) -> Option<usize> {
        self.logical_types
            .iter()
            .map(|(n, k)| (n.as_str(), *k))
            .chain(LIBRARY_LOGICAL.iter().copied())
            .find(|(n, _)| *n == name)
            .map(|(_, k)| k)
    }

    fn trait_info(&self, name: &str) -> Option<&TraitInfo> {
        self.traits.iter().find(|t| t.name == name)
    }

    /// Whether a trait of this invocation has a method that runs, its
    /// supertraits of this invocation included. `None` for any other trait.
    fn trait_runs(&self, name: &str) -> Option<bool> {
        let t = self.trait_info(name)?;
        Some(t.runs || t.supers.iter().any(|s| self.trait_runs(s) == Some(true)))
    }
}

/// Where an item stands.
#[derive(Clone, Default)]
struct Place {
    /// Inside an `impl` or a `trait`: the stub of a logic function there has
    /// a mangled name.
    in_assoc: bool,
    /// Type parameters in scope that are bounded by `Logical`, and `Self`
    /// where it is a logical type.
    logical_params: Vec<String>,
    /// For a stub that stands outside its trait: the trait, and the
    /// associated types that `Self::Name` must name through it.
    qualify: Option<(String, Vec<String>)>,
    /// A predicate to add to the stub's `where` clause.
    extra_where: Option<String>,
}

/// The bindings of a runtime body, by lexical scope.
#[derive(Clone, Default)]
struct Scope {
    /// Bindings shown to be ghost.
    ghosts: Vec<String>,
    /// Ghost locals that only logical code mentions: they hold `Untyped`.
    untyped: Vec<String>,
    /// Every parameter and local, so that a call of one in logical code is
    /// not probed as an item.
    locals: Vec<String>,
    logical_params: Vec<String>,
}

impl Scope {
    fn bind(&mut self, names: &[String], ghost: bool) {
        for n in names {
            self.ghosts.retain(|g| g != n);
            self.untyped.retain(|g| g != n);
        }
        if ghost {
            self.ghosts.extend(names.iter().cloned());
        }
        self.locals.extend(names.iter().cloned());
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
        cx.members_in_trait |= text.contains("logic_members_in_trait");
        cx.allow_ghost_headers |= text.contains("allow_ghost_headers");
        i += 3;
    }
    i
}

/// The logical types and the traits of this invocation, inline modules
/// included. GUESS: names are not qualified by their module.
fn collect_items(toks: &[TT], cx: &mut Cx) {
    for item in split_items(toks) {
        let (_, derive_logical, i) = take_attrs(item);
        let rest = &item[skip_vis(item, i)..];
        let first = ident_name(rest.first()).unwrap_or_default();
        if derive_logical && (first == "struct" || first == "enum") {
            if let Some(name) = ident_name(rest.get(1)) {
                let (generics, _) = generics_at(rest, 2);
                cx.logical_types.push((name, generic_names(generics).len()));
            }
        } else if first == "mod" {
            if let Some(body) = group(rest.get(2), Delimiter::Brace) {
                collect_items(&collect(body.stream()), cx);
            }
        } else if let Some(t) = rest.iter().take(3).position(|t| ident(Some(t), "trait")) {
            let (Some(name), Some(TT::Group(body))) = (ident_name(rest.get(t + 1)), rest.last())
            else {
                continue;
            };
            let (_, after) = generics_at(rest, t + 2);
            let mut info = TraitInfo {
                name,
                ..TraitInfo::default()
            };
            for t in &rest[after..rest.len() - 1] {
                if ident(Some(t), "where") {
                    break;
                }
                info.supers.extend(ident_name(Some(t)));
            }
            let inner = collect(body.stream());
            for member in split_items(&inner) {
                let (_, _, a) = take_attrs(member);
                let m = &member[skip_vis(member, a)..];
                if ident(m.first(), "logic") && ident(m.get(1), "fn") {
                    info.has_logic = true;
                } else if m.iter().take(4).any(|t| ident(Some(t), "fn")) {
                    info.runs = true;
                }
            }
            cx.traits.push(info);
        }
    }
}

/// An error of `locus!` itself, reported at `span`.
fn error(span: Span, msg: &str) -> TokenStream {
    let mut out = parse("::core::compile_error!", span);
    let mut lit = TT::Literal(proc_macro2::Literal::string(&format!("locus: {msg}")));
    lit.set_span(span);
    let mut g = Group::new(Delimiter::Brace, TokenStream::from(lit));
    g.set_span(span);
    out.extend([TT::Group(g)]);
    out
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

fn lower_item(item: &[TT], cx: &Cx, place: &Place) -> TokenStream {
    let (attrs, derive_logical, i) = take_attrs(item);
    let vis = lower_vis(item, i);
    let rest = &item[skip_vis(item, i)..];
    let mut out = TokenStream::new();
    let first = ident_name(rest.first()).unwrap_or_default();
    // `logic fn`: a contextual keyword, so `r#logic` and `fn logic` are not it.
    if first == "logic" && ident(rest.get(1), "fn") {
        return lower_fn(attrs, vis, &rest[1..], cx, true, place);
    }
    if first == "prop"
        && ident_name(rest.get(1)).is_some()
        && group(rest.get(2), Delimiter::Parenthesis).is_some()
    {
        return lower_prop(vis, rest, cx);
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
        Some("fn") if qualifiers_only("fn") => lower_fn(attrs, vis, rest, cx, false, place),
        Some(k @ ("struct" | "enum")) if derive_logical && qualifiers_only(k) => {
            lower_logical_type(attrs, vis, rest)
        }
        Some(k @ ("struct" | "enum" | "union")) if qualifiers_only(k) => {
            out.extend(attrs);
            out.extend(vis);
            out.extend(lower_adt(rest));
            out
        }
        Some("impl") if qualifiers_only("impl") => lower_impl(attrs, vis, rest, cx),
        Some("trait") if qualifiers_only("trait") => lower_trait(attrs, vis, rest, cx),
        Some("mod")
            if ident(rest.first(), "mod") && group(rest.get(2), Delimiter::Brace).is_some() =>
        {
            let body = group(rest.get(2), Delimiter::Brace).unwrap();
            let inner = collect(body.stream());
            let mut lowered = TokenStream::new();
            for item in split_items(&inner) {
                lowered.extend(lower_item(item, cx, &Place::default()));
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
            // `use`, `const`, `macro_rules!`, other macros' invocations:
            // copied as written. A macro's input is never rewritten. A `type`
            // item may write a proof type: `type Out = @(1 == 1);`.
            out.extend(attrs);
            out.extend(vis);
            if first == "type" {
                out.extend(lower_type(rest));
            } else {
                copy(&mut out, rest);
            }
            out
        }
    }
}

// ------------------------------------------------------------------- generics

/// `<...>` starting at `toks[i]`, with its angle brackets, and the index
/// after it. Empty when there is none.
fn generics_at(toks: &[TT], i: usize) -> (&[TT], usize) {
    if !punct(toks.get(i), '<') {
        return (&toks[i.min(toks.len())..i.min(toks.len())], i);
    }
    let mut depth = 0;
    for k in i..toks.len() {
        if punct(toks.get(k), '<') {
            depth += 1;
        } else if punct(toks.get(k), '>') && !thin_arrow(toks, k - 1) && !fat_arrow(toks, k - 1) {
            depth -= 1;
            if depth == 0 {
                return (&toks[i..=k], k + 1);
            }
        }
    }
    (&toks[i..], toks.len())
}

/// The parameters of `<...>`, one slice each.
fn generic_parts(generics: &[TT]) -> Vec<&[TT]> {
    if generics.len() < 2 {
        return Vec::new();
    }
    split_top(&generics[1..generics.len() - 1], ',')
        .into_iter()
        .map(|(part, _)| part)
        .collect()
}

/// The names of the parameters: `'a`, `T`, `N`.
fn generic_names(generics: &[TT]) -> Vec<String> {
    generic_parts(generics)
        .into_iter()
        .filter_map(|part| {
            if punct(part.first(), '\'') {
                ident_name(part.get(1)).map(|n| format!("'{n}"))
            } else if ident(part.first(), "const") {
                ident_name(part.get(1))
            } else {
                ident_name(part.first())
            }
        })
        .collect()
}

/// `<'a, T, N>`, the parameters used as arguments, or nothing.
fn generic_args(generics: &[TT]) -> String {
    let names = generic_names(generics);
    if names.is_empty() {
        String::new()
    } else {
        format!("<{}>", names.join(", "))
    }
}

/// A parameter without its default: `Rhs: Logical` for `Rhs: Logical = Self`.
fn without_default(part: &[TT]) -> &[TT] {
    let mut depth = 0;
    for c in 0..part.len() {
        if punct(part.get(c), '<') {
            depth += 1;
        } else if punct(part.get(c), '>') {
            depth -= 1;
        } else if depth == 0 && single_eq(part, c) {
            return &part[..c];
        }
    }
    part
}

/// Type parameters whose bounds, written in the parameter list, name
/// `Logical`. GUESS: a bound in a `where` clause is not read, and neither is
/// a bound on a trait that has `Logical` as a supertrait, such as
/// `T: AddLogic`: that needs the trait's declaration.
fn logical_params(generics: &[TT]) -> Vec<String> {
    generic_parts(generics)
        .into_iter()
        .filter(|part| {
            let part = without_default(part);
            part.iter()
                .position(|t| punct(Some(t), ':'))
                .is_some_and(|c| part[c..].iter().any(|t| ident(Some(t), "Logical")))
        })
        .filter_map(|part| ident_name(part.first()))
        .collect()
}

// ------------------------------------------------------------------ functions

/// The name of a logic function's stub inside an `impl` or a trait. No Rust
/// code can write it, so the stub is never a candidate when rustc resolves a
/// name that Rust code wrote.
fn mangled(name: &str) -> String {
    format!("__logic_{name}")
}

/// A runtime `fn`, or a `logic fn` (`logic` already removed), which becomes
/// a stub: its signature, and a body that probes the names its logical code
/// calls and then panics.
fn lower_fn(
    attrs: TokenStream,
    vis: TokenStream,
    toks: &[TT],
    cx: &Cx,
    logic: bool,
    place: &Place,
) -> TokenStream {
    let fn_at = toks.iter().position(|t| ident(Some(t), "fn")).unwrap();
    let span = toks[fn_at].span();
    let (generics, j) = generics_at(toks, fn_at + 2);
    let params = group(toks.get(j), Delimiter::Parenthesis).expect("toy: fn without parameters");
    let body_at = match toks.last() {
        Some(TT::Group(g)) if g.delimiter() == Delimiter::Brace => toks.len() - 1,
        _ => toks.len(),
    };
    let mut sc = Scope {
        logical_params: place.logical_params.clone(),
        ..Scope::default()
    };
    sc.logical_params.extend(logical_params(generics));
    let mut out = TokenStream::new();
    out.extend(attrs);
    if logic {
        out.extend(parse(
            "#[allow(dead_code, non_snake_case, unused_variables)]",
            span,
        ));
    }
    out.extend(vis);
    copy(&mut out, &toks[..=fn_at]);
    match &toks[fn_at + 1] {
        TT::Ident(id) if logic && place.in_assoc => {
            out.extend([TT::Ident(Ident::new(&mangled(&id.to_string()), id.span()))]);
        }
        t => out.extend([t.clone()]),
    }
    copy(&mut out, generics);
    let qualify = |ts: TokenStream| match &place.qualify {
        Some((trait_ref, assoc)) if logic => qualify_self(ts, trait_ref, assoc),
        _ => ts,
    };
    out.extend([regroup(
        params,
        qualify(lower_params(params, cx, logic, &mut sc)),
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
        out.extend(qualify(lower_type(&tail[2..k])));
    }
    if let (Some(extra), true) = (&place.extra_where, logic) {
        out.extend(parse(&format!("where {extra}"), span));
        if ident(tail.get(k), "where") {
            out.extend(parse(",", span));
            k += 1;
        }
    }
    copy(&mut out, &tail[k..]);
    if let Some(TT::Group(body)) = toks.get(body_at) {
        if logic {
            // Parameters and the binders of logical closures are not items.
            let mut bound = sc.locals.clone();
            find_places(body.stream(), &mut bound, &mut Vec::new());
            let mut probes = Probes::default();
            find_items(body.stream(), cx, &bound, true, &mut probes);
            let mut stub = parse("::locus::stub_body!", body.span());
            let mut g = Group::new(Delimiter::Parenthesis, probes.groups(body.span()));
            g.set_span(body.span());
            stub.extend([TT::Group(g)]);
            out.extend([regroup(body, stub)]);
        } else {
            out.extend([regroup(
                body,
                lower_seq(&collect(body.stream()), cx, &mut sc),
            )]);
        }
    }
    out
}

/// `Self::Output` is `<Self as Trait>::Output`, for the associated types of
/// the trait whose logic functions stand outside it.
fn qualify_self(ts: TokenStream, trait_ref: &str, assoc: &[String]) -> TokenStream {
    let toks = collect(ts);
    let mut out = TokenStream::new();
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            TT::Ident(id)
                if id == "Self"
                    && punct(toks.get(i + 1), ':')
                    && punct(toks.get(i + 2), ':')
                    && ident_name(toks.get(i + 3)).is_some_and(|n| assoc.contains(&n)) =>
            {
                out.extend(parse(&format!("<Self as {trait_ref}>::"), id.span()));
                out.extend([toks[i + 3].clone()]);
                i += 4;
            }
            // `Proof![..]` holds a proposition, which is not a type for rustc.
            TT::Group(g)
                if g.delimiter() == Delimiter::Bracket
                    && punct(toks.get(i.wrapping_sub(1)), '!') =>
            {
                out.extend([toks[i].clone()]);
                i += 1;
            }
            TT::Group(g) => {
                out.extend([regroup(g, qualify_self(g.stream(), trait_ref, assoc))]);
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

fn lower_params(params: &Group, cx: &Cx, logic: bool, sc: &mut Scope) -> TokenStream {
    let toks = collect(params.stream());
    let mut out = TokenStream::new();
    for (part, sep) in split_top(&toks, ',') {
        let (attrs, _, a) = take_attrs(part);
        match (a..part.len()).find(|&c| single_colon(part, c)) {
            Some(c) => {
                let ty = &part[c + 1..];
                let ghost = punct(ty.first(), '@') || is_logical_type(ty, cx, &sc.logical_params);
                out.extend(attrs);
                if ghost && !logic {
                    // A ghost parameter: lowering marks it for the runtime lints.
                    out.extend(parse("#[allow(unused_variables)]", part[c].span()));
                }
                sc.bind(&pattern_binders(&part[a..c]), ghost);
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

/// Whether a type written in the source is shown to be logical. GUESS: by
/// the name it starts with. `Vec<Nat>`, `&Nat`, an alias, and an associated
/// type such as `T::Out` are not recognized.
fn is_logical_type(ty: &[TT], cx: &Cx, logical_params: &[String]) -> bool {
    match ident_name(ty.first()) {
        Some(n) => n == "Snap" || cx.logical(&n).is_some() || logical_params.contains(&n),
        None => false,
    }
}

/// The names a pattern binds. GUESS: every lowercase identifier that is not
/// a field label, a path segment, or a call. Whether `limit` in a pattern is
/// a binding or a constant in scope needs name resolution.
fn pattern_binders(pat: &[TT]) -> Vec<String> {
    let mut names = Vec::new();
    for (i, t) in pat.iter().enumerate() {
        match t {
            TT::Group(g) => names.extend(pattern_binders(&collect(g.stream()))),
            TT::Ident(id) => {
                let n = id.to_string();
                let path = punct(pat.get(i + 1), ':') && punct(pat.get(i + 2), ':')
                    || (i > 0 && punct(pat.get(i - 1), ':'));
                let call = any_group(pat.get(i + 1)).is_some() || punct(pat.get(i + 1), '!');
                if n != "mut"
                    && n != "ref"
                    && n != "_"
                    && lowercase(&n)
                    && !single_colon(pat, i + 1)
                    && !path
                    && !call
                {
                    names.push(n);
                }
            }
            _ => {}
        }
    }
    names
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

/// `#[derive(Logical)]`: a zero-sized struct of its own, under the type's
/// own name and generic where the type is, with `Clone`, `Copy`, `Debug` and
/// `Logical`, and a stub function for each variant. Its fields are logical
/// and are not written: the struct holds only its parameters, in a
/// `PhantomData<fn() -> (T,)>`, so it owns none of them. The checking
/// expansion names the types of the fields in a function beside it.
fn lower_logical_type(attrs: TokenStream, vis: TokenStream, toks: &[TT]) -> TokenStream {
    let is_enum = ident(toks.first(), "enum");
    let name = ident_name(toks.get(1)).unwrap();
    let span = toks[1].span();
    let (generics, _) = generics_at(toks, 2);
    if let Some(t) = generics.iter().find(|t| punct(Some(t), '\'')) {
        return error(
            t.span(),
            &format!(
                "the logical type `{name}` declares a lifetime parameter: no marker carries a lifetime"
            ),
        );
    }
    let parts = generic_parts(generics);
    let names = generic_names(generics);
    let text = |part: &[TT]| part.iter().cloned().collect::<TokenStream>().to_string();
    // The parameters as declared; without defaults, for an `impl`; and with
    // `Logical` added to each bound, for the implementation of `Logical`.
    let (mut decl, mut plain, mut bounded) = (Vec::new(), Vec::new(), Vec::new());
    for part in &parts {
        let p = without_default(part);
        decl.push(text(part));
        plain.push(text(p));
        bounded.push(if ident(p.first(), "const") {
            text(p)
        } else if p.iter().any(|t| punct(Some(t), ':')) {
            format!("{} + ::locus::Logical", text(p))
        } else {
            format!("{}: ::locus::Logical", text(p))
        });
    }
    let wrap = |v: &[String]| {
        if v.is_empty() {
            String::new()
        } else {
            format!("<{}>", v.join(", "))
        }
    };
    let (decl, plain, bounded, args) = (wrap(&decl), wrap(&plain), wrap(&bounded), wrap(&names));
    let types: Vec<&String> = names
        .iter()
        .zip(&parts)
        .filter(|(_, p)| !ident(p.first(), "const"))
        .map(|(n, _)| n)
        .collect();
    let (field, value) = if types.is_empty() {
        (String::new(), String::new())
    } else {
        let list: String = types.iter().map(|n| format!("{n}, ")).collect();
        (
            format!("__locus: ::core::marker::PhantomData<fn() -> ({list})>"),
            "__locus: ::core::marker::PhantomData".to_string(),
        )
    };
    let vis_s = vis.to_string();
    let mut s = format!(
        "#[allow(dead_code)] {vis_s} struct {name}{decl} {{ {field} }}
         impl{plain} ::core::clone::Clone for {name}{args} {{ fn clone(&self) -> Self {{ *self }} }}
         impl{plain} ::core::marker::Copy for {name}{args} {{}}
         impl{plain} ::core::fmt::Debug for {name}{args} {{
             fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {{
                 f.write_str(\"{name}\")
             }}
         }}
         impl{bounded} ::locus::Logical for {name}{args} {{
             fn __locus_marker() -> Self {{ {name} {{ {value} }} }}
         }}"
    );
    // The types of the fields, for a probe: the checking expansion names
    // them, so that rustc resolves them, the type itself among them.
    let mut field_types: Vec<String> = Vec::new();
    let body = toks
        .last()
        .and_then(|t| any_group(Some(t)))
        .map(|g| collect(g.stream()));
    if is_enum {
        let body = body.unwrap_or_default();
        let mut fns = String::new();
        for (part, _) in split_top(&body, ',') {
            let Some(variant) = ident_name(part.first()) else {
                continue;
            };
            let params = match group(part.get(1), Delimiter::Parenthesis) {
                Some(g) => split_top(&collect(g.stream()), ',')
                    .into_iter()
                    .map(|(ty, _)| {
                        field_types.push(lower_type(ty).to_string());
                        format!("_: {}", lower_type(ty))
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
                None => String::new(),
            };
            fns.push_str(&format!(
                "{vis_s} fn {variant}({params}) -> {name}{args} {{ ::locus::stub() }}\n"
            ));
        }
        s.push_str(&format!(
            "#[allow(non_snake_case, dead_code)] impl{plain} {name}{args} {{ {fns} }}"
        ));
    } else if let Some(body) = body {
        for (part, _) in split_top(&body, ',') {
            let (_, _, a) = take_attrs(part);
            let from = (a..part.len())
                .find(|&c| single_colon(part, c))
                .map_or(skip_vis(part, a), |c| c + 1);
            field_types.push(lower_type(&part[from..]).to_string());
        }
    }
    if !field_types.is_empty() {
        let list: String = field_types.iter().map(|t| format!("{t}, ")).collect();
        s.push_str(&format!(
            "#[cfg(__locus_check)] #[allow(non_snake_case, dead_code)]
             fn __locus_fields_{name}{plain}() {{ let _: ::core::marker::PhantomData<({list})>; }}"
        ));
    }
    let mut out = attrs;
    out.extend(parse(&s, span));
    out
}

/// `prop Name(params) { Arm(fields) => { .. } .. }`: a stub function for the
/// proposition, and a field-less struct of the same name, in the type
/// namespace only, whose stub functions are the named arms.
fn lower_prop(vis: TokenStream, toks: &[TT], cx: &Cx) -> TokenStream {
    let name = ident_name(toks.get(1)).unwrap();
    let span = toks[1].span();
    let params = group(toks.get(2), Delimiter::Parenthesis).unwrap();
    let mut sc = Scope::default();
    let lowered_params = lower_params(params, cx, true, &mut sc);
    let vis_s = vis.to_string();
    let mut arms = String::new();
    if let Some(body) = group(toks.get(3), Delimiter::Brace) {
        let body = collect(body.stream());
        let mut i = 0;
        while i < body.len() {
            if let Some(arm) = ident_name(body.get(i)) {
                let args = match group(body.get(i + 1), Delimiter::Parenthesis) {
                    Some(g) => lower_params(g, cx, true, &mut sc).to_string(),
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

// ---------------------------------------------------------------------- traits

/// The hidden trait that holds the stubs of a trait's logic functions.
fn companion(name: &str) -> String {
    format!("__locus_logic_{name}")
}

const STUB_LINTS: &str =
    "#[doc(hidden)] #[allow(non_camel_case_types, non_snake_case, dead_code, unused_variables)]";

/// Whether a member of a trait or an `impl` is a `logic fn`; and the index of
/// `logic`.
fn logic_member(item: &[TT]) -> Option<usize> {
    let (_, _, a) = take_attrs(item);
    let v = skip_vis(item, a);
    (ident(item.get(v), "logic") && ident(item.get(v + 1), "fn")).then_some(v)
}

/// Whether a member is a function that runs; and its name.
fn runtime_member(item: &[TT]) -> Option<&TT> {
    if logic_member(item).is_some() {
        return None;
    }
    let (_, _, a) = take_attrs(item);
    let v = skip_vis(item, a);
    let f = (v..item.len().min(v + 5)).find(|&k| ident(item.get(k), "fn"))?;
    item[v..f]
        .iter()
        .all(|t| {
            matches!(t, TT::Literal(_))
                || ["const", "unsafe", "async", "extern", "default", "safe"]
                    .iter()
                    .any(|q| ident(Some(t), q))
        })
        .then(|| &item[f + 1])
}

/// The associated types that the members of a trait or an `impl` declare.
fn assoc_types(members: &[&[TT]]) -> Vec<String> {
    members
        .iter()
        .filter_map(|m| {
            let (_, _, a) = take_attrs(m);
            let v = skip_vis(m, a);
            if ident(m.get(v), "type") {
                ident_name(m.get(v + 1))
            } else {
                None
            }
        })
        .collect()
}

/// A trait. A logic function is not an item of the lowered trait: its stub
/// goes to a hidden trait declared beside it, `__locus_logic_Name`, with the
/// same parameters and supertraits, which the lowered trait has as one more
/// supertrait. So `T: Name` gives rustc the stubs for `T`, and an `impl` of
/// `Name` must come with an `impl` of the hidden trait.
///
/// With `#![spike(logic_members_in_trait)]`, the stubs stay in the trait.
///
/// Either way a stub has a mangled name, `__logic_name`.
fn lower_trait(attrs: TokenStream, vis: TokenStream, toks: &[TT], cx: &Cx) -> TokenStream {
    let mut out = TokenStream::new();
    let t_at = toks.iter().position(|t| ident(Some(t), "trait")).unwrap();
    let (Some(name), Some(TT::Group(body))) = (ident_name(toks.get(t_at + 1)), toks.last()) else {
        out.extend(attrs);
        out.extend(vis);
        copy(&mut out, toks);
        return out;
    };
    let span = toks[t_at + 1].span();
    let (generics, after) = generics_at(toks, t_at + 2);
    let bounds = &toks[after..toks.len() - 1];
    let args = generic_args(generics);
    let inner = collect(body.stream());
    let members = split_items(&inner);
    let assoc = assoc_types(&members);
    // `trait Monoid: Logical`: `Self` is a logical type in its members.
    let mut logical = logical_params(generics);
    if bounds
        .iter()
        .take_while(|t| !ident(Some(t), "where"))
        .any(|t| ident(Some(t), "Logical"))
    {
        logical.push("Self".to_string());
    }
    let runtime = Place {
        in_assoc: true,
        logical_params: logical.clone(),
        ..Place::default()
    };
    let stub = if cx.members_in_trait {
        Place {
            extra_where: Some("Self: ::core::marker::Sized".to_string()),
            ..runtime.clone()
        }
    } else {
        Place {
            qualify: Some((format!("{name}{args}"), assoc)),
            extra_where: Some(format!("Self: ::core::marker::Sized + {name}{args}")),
            ..runtime.clone()
        }
    };
    let (mut kept, mut stubs) = (TokenStream::new(), TokenStream::new());
    for member in members {
        match logic_member(member) {
            Some(v) => {
                let (a, _, _) = take_attrs(member);
                let lowered = lower_fn(a, TokenStream::new(), &member[v + 1..], cx, true, &stub);
                if cx.members_in_trait {
                    kept.extend(lowered);
                } else {
                    stubs.extend(lowered);
                }
            }
            None => kept.extend(lower_item(member, cx, &runtime)),
        }
    }
    if !stubs.is_empty() {
        let hidden = companion(&name);
        out.extend(parse(STUB_LINTS, span));
        out.extend(vis.clone());
        out.extend(parse(&format!("trait {hidden}"), span));
        copy(&mut out, generics);
        copy(&mut out, bounds);
        out.extend([regroup(body, stubs)]);
    }
    out.extend(attrs);
    out.extend(vis);
    copy(&mut out, &toks[..after]);
    if cx.traits.iter().any(|t| t.name == name && t.has_logic) && !cx.members_in_trait {
        // One more supertrait, written first.
        out.extend(parse(&format!(": {}{args}", companion(&name)), span));
        match bounds.first() {
            Some(_) if single_colon(bounds, 0) => {
                if bounds.len() > 1 && !ident(bounds.get(1), "where") {
                    out.extend(parse("+", span));
                }
                copy(&mut out, &bounds[1..]);
            }
            _ => copy(&mut out, bounds),
        }
    } else {
        copy(&mut out, bounds);
    }
    out.extend([regroup(body, kept)]);
    out
}

/// A proof type, a dependent tuple, or a snapshot type in the header of an
/// `impl`. rustc sees every proof as `Erased` and every snapshot as one
/// marker, so such an implementation would apply more widely than it says.
/// GUESS: a snapshot type is known by the name `Snap` or `CutSnap`; an alias
/// of one is not seen.
fn ghost_in_header(toks: &[TT]) -> Option<(Span, &'static str)> {
    for (i, t) in toks.iter().enumerate() {
        match t {
            TT::Punct(p) if p.as_char() == '@' => return Some((p.span(), "a proof type")),
            TT::Ident(id) if id == "Snap" || id == "CutSnap" => {
                return Some((id.span(), "a snapshot type"));
            }
            TT::Group(g) => {
                let inner = collect(g.stream());
                if g.delimiter() == Delimiter::Parenthesis
                    && split_top(&inner, ',').iter().any(|(part, _)| {
                        ident_name(part.first()).is_some() && single_colon(part, 1)
                    })
                {
                    return Some((g.span(), "a dependent tuple"));
                }
                if let Some(found) = ghost_in_header(&inner) {
                    return Some(found);
                }
            }
            _ => {}
        }
        let _ = i;
    }
    None
}

/// An `impl`. Its header is checked, a logical type gets no method that
/// runs, and the logic functions of an `impl` of a trait go to an `impl` of
/// the hidden trait that holds the trait's stubs.
fn lower_impl(attrs: TokenStream, vis: TokenStream, toks: &[TT], cx: &Cx) -> TokenStream {
    let mut out = TokenStream::new();
    let Some(TT::Group(body)) = toks.last() else {
        out.extend(attrs);
        out.extend(vis);
        copy(&mut out, toks);
        return out;
    };
    let header = &toks[..toks.len() - 1];
    let impl_at = header.iter().position(|t| ident(Some(t), "impl")).unwrap();
    let (generics, after) = generics_at(header, impl_at + 1);
    let where_at = (after..header.len())
        .find(|&k| ident(header.get(k), "where"))
        .unwrap_or(header.len());
    // `Trait<Args> for Type`, or `Type`. A `for<'a>` is not this `for`.
    let mut depth = 0;
    let mut for_at = None;
    for k in after..where_at {
        if punct(header.get(k), '<') {
            depth += 1;
        } else if punct(header.get(k), '>') && !thin_arrow(header, k - 1) {
            depth -= 1;
        } else if depth == 0 && ident(header.get(k), "for") && !punct(header.get(k + 1), '<') {
            for_at = Some(k);
            break;
        }
    }
    let self_ty = &header[for_at.map_or(after, |f| f + 1)..where_at];
    let trait_ref = for_at.map(|f| &header[after..f]);
    // The trait's name: the last identifier outside `<...>`.
    let trait_name_at = for_at.and_then(|f| {
        let mut depth = 0;
        let mut last = None;
        for k in after..f {
            if punct(header.get(k), '<') {
                depth += 1;
            } else if punct(header.get(k), '>') {
                depth -= 1;
            } else if depth == 0 && ident_name(header.get(k)).is_some() {
                last = Some(k);
            }
        }
        last
    });
    let trait_name = trait_name_at.and_then(|k| ident_name(header.get(k)));

    if !cx.allow_ghost_headers {
        if let Some((span, what)) = ghost_in_header(&header[after..where_at]) {
            return error(
                span,
                &format!(
                    "{what} is written in the header of an `impl`: rustc would apply it to every proof, or to every snapshot"
                ),
            );
        }
    }

    let mut logical = logical_params(generics);
    let self_logical = is_logical_type(self_ty, cx, &logical);
    if self_logical {
        logical.push("Self".to_string());
    }
    let inner = collect(body.stream());
    let members = split_items(&inner);
    if let (Some(tr), true) = (&trait_name, self_logical) {
        // A logical type implements only traits with no method that runs.
        // From the `impl` alone, that is known only of a method it gives;
        // a method the trait provides is known only if the trait is declared
        // in this invocation.
        if let Some(method) = members.iter().find_map(|m| runtime_member(m)) {
            return error(
                method.span(),
                &format!(
                    "a logical type implements only traits with no method that runs, and this implementation of `{tr}` gives the method `{method}`"
                ),
            );
        }
        if cx.trait_runs(tr) == Some(true) {
            return error(
                header[trait_name_at.unwrap()].span(),
                &format!(
                    "a logical type implements only traits with no method that runs, and `{tr}` has one"
                ),
            );
        }
    }
    let runtime = Place {
        in_assoc: true,
        logical_params: logical,
        ..Place::default()
    };
    let has_logic = members.iter().any(|m| logic_member(m).is_some());
    // Whether the trait has a hidden trait for its stubs. GUESS, for a trait
    // that this invocation does not declare: it has one exactly when this
    // `impl` gives a logic function.
    let split = !cx.members_in_trait
        && match &trait_name {
            Some(tr) => cx.trait_info(tr).map_or(has_logic, |t| t.has_logic),
            None => false,
        };
    let (mut kept, mut stubs) = (TokenStream::new(), TokenStream::new());
    let stub = Place {
        qualify: match (split, trait_ref) {
            (true, Some(tr)) => Some((
                tr.iter().cloned().collect::<TokenStream>().to_string(),
                assoc_types(&members),
            )),
            _ => None,
        },
        ..runtime.clone()
    };
    for member in &members {
        if split && logic_member(member).is_some() {
            stubs.extend(lower_item(member, cx, &stub));
        } else {
            kept.extend(lower_item(member, cx, &runtime));
        }
    }
    if split {
        let at = trait_name_at.unwrap();
        out.extend(parse(STUB_LINTS, header[at].span()));
        let renamed: Vec<TT> = header
            .iter()
            .enumerate()
            .map(|(k, t)| match t {
                TT::Ident(id) if k == at => {
                    TT::Ident(Ident::new(&companion(&id.to_string()), id.span()))
                }
                t => t.clone(),
            })
            .collect();
        out.extend(lower_type(&renamed));
        out.extend([regroup(body, stubs)]);
    }
    out.extend(attrs);
    out.extend(vis);
    // A proof type in the header, where that is not rejected, is `Erased`.
    out.extend(lower_type(header));
    out.extend([regroup(body, kept)]);
    out
}

// ---------------------------------------------------------------------- bodies

/// A proof hole: `_` in an expression.
fn hole(span: Span) -> TokenStream {
    parse("::locus::proof!([] [] [])", span)
}

fn closure_start(prev: Option<&TT>) -> bool {
    match prev {
        None => true,
        Some(TT::Punct(p)) => p.as_char() != '?',
        Some(TT::Ident(i)) => i == "move" || i == "return",
        _ => false,
    }
}

fn locus_macro_at(toks: &[TT], i: usize) -> Option<&'static str> {
    if !punct(toks.get(i + 1), '!') || any_group(toks.get(i + 2)).is_none() {
        return None;
    }
    match ident_name(toks.get(i)).as_deref() {
        Some("prove") => Some("prove"),
        Some("logic") => Some("logic"),
        Some("prop") => Some("prop"),
        _ => None,
    }
}

/// The library macro for a Locus macro written in a body. `logic!` gives an
/// untyped marker only where it initializes, or is assigned to, a ghost
/// local that only logical code mentions.
fn kind_of(name: &str, untyped: bool) -> &'static str {
    match name {
        "prove" => "proof",
        "prop" => "prop_value",
        _ if untyped => "untyped",
        _ => "data",
    }
}

/// Statements and expressions of a runtime body.
fn lower_seq(toks: &[TT], cx: &Cx, sc: &mut Scope) -> TokenStream {
    let mut out = TokenStream::new();
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        let prev = if i > 0 { toks.get(i - 1) } else { None };
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
                        let rest = &toks[(end + 1).min(toks.len())..];
                        out.extend(lower_let(&toks[i..end], toks.get(end), rest, cx, sc));
                        i = end + 1;
                        continue;
                    }
                    "match" => {
                        out.extend([t.clone()]);
                        let j = (i + 1..toks.len())
                            .find(|&k| group(toks.get(k), Delimiter::Brace).is_some())
                            .unwrap_or(toks.len());
                        out.extend(lower_seq(&toks[i + 1..j], cx, sc));
                        if let Some(TT::Group(arms)) = toks.get(j) {
                            out.extend([regroup(
                                arms,
                                lower_arms(&collect(arms.stream()), cx, sc),
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
                    _ if locus_macro_at(toks, i).is_some() => {
                        out.extend(lower_logic(
                            kind_of(&s, false),
                            any_group(toks.get(i + 2)).unwrap(),
                            id.span(),
                            cx,
                            sc,
                        ));
                        i += 3;
                        continue;
                    }
                    _ if single_eq(toks, i + 1)
                        && !punct(prev, '.')
                        && locus_macro_at(toks, i + 2).is_some() =>
                    {
                        // A ghost statement: `seen = logic! { seen + 1 };`.
                        let name = locus_macro_at(toks, i + 2).unwrap();
                        copy(&mut out, &toks[i..i + 2]);
                        out.extend(lower_logic(
                            kind_of(name, sc.untyped.contains(&s)),
                            any_group(toks.get(i + 4)).unwrap(),
                            toks[i + 2].span(),
                            cx,
                            sc,
                        ));
                        i += 5;
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
                let (types, next) = generics_at(toks, i + 2);
                out.extend(lower_type(types));
                i = next;
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
                            let mut s = lower_seq(&inner[..k], cx, sc);
                            copy(&mut s, &inner[k..]);
                            s
                        }
                        None => lower_seq(&inner, cx, sc),
                    }
                } else if g.delimiter() == Delimiter::Brace {
                    let mut scope = sc.clone();
                    lower_seq(&inner, cx, &mut scope)
                } else {
                    lower_seq(&inner, cx, sc)
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

/// One arm of a `match`: its pattern, its guard (from `if`), `=>`, its body,
/// and the comma after it.
struct Arm<'a> {
    pat: &'a [TT],
    guard: &'a [TT],
    arrow: &'a [TT],
    body: &'a [TT],
    comma: Option<&'a TT>,
}

fn split_arms(toks: &[TT]) -> Vec<Arm<'_>> {
    let mut arms = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let start = i;
        while i < toks.len() && !fat_arrow(toks, i) {
            i += 1;
        }
        let head = &toks[start..i];
        let split = head
            .iter()
            .position(|t| ident(Some(t), "if"))
            .unwrap_or(head.len());
        let arrow = &toks[i.min(toks.len())..(i + 2).min(toks.len())];
        i = (i + 2).min(toks.len());
        let s = i;
        if group(toks.get(i), Delimiter::Brace).is_some() {
            i += 1;
        } else {
            while i < toks.len() && !punct(toks.get(i), ',') {
                i += 1;
            }
        }
        let body = &toks[s..i];
        let comma = toks.get(i).filter(|t| punct(Some(t), ','));
        if comma.is_some() {
            i += 1;
        }
        arms.push(Arm {
            pat: &head[..split],
            guard: &head[split..],
            arrow,
            body,
            comma,
        });
    }
    arms
}

fn lower_arms(toks: &[TT], cx: &Cx, sc: &mut Scope) -> TokenStream {
    let mut out = TokenStream::new();
    for arm in split_arms(toks) {
        // What the pattern binds is a runtime local in this arm.
        let mut scope = sc.clone();
        scope.bind(&pattern_binders(arm.pat), false);
        copy(&mut out, arm.pat);
        out.extend(lower_seq(arm.guard, cx, &mut scope));
        copy(&mut out, arm.arrow);
        out.extend(lower_seq(arm.body, cx, &mut scope));
        out.extend(arm.comma.cloned());
    }
    out
}

/// Whether runtime code in `toks`, the rest of a block after a ghost `let`,
/// mentions the local `name`. This decides the draft's rule: a ghost local
/// that only logical code mentions holds an untyped marker.
///
/// Logical code is what `locus!` itself wraps: `prove!`, `logic!` and
/// `prop!` written in the body, propositions in a `let`'s annotation, and a
/// ghost statement `name = logic! { .. };`. The walk follows Rust's scopes,
/// so a later binding of the same name ends it.
///
/// GUESS, each from the tokens alone:
/// - a mention in another macro's input counts as runtime code, unless it is
///   inside a `prove!`, `logic!` or `prop!` there. What that macro does with
///   its input is not known. A name inside a string literal, as in
///   `format!("{g:?}")`, is not seen at all;
/// - `prove!`, `logic!` and `prop!` are known by those names, not by what
///   they resolve to;
/// - what a pattern binds is every lowercase identifier in it;
/// - a closure's body ends at the next `,` or `;`, or is a block;
/// - an operator on bindings shown to be ghost, and a call of a logic
///   function, are logical code outside any macro (RFC 0002), and are taken
///   for runtime code here.
fn runtime_mention(toks: &[TT], name: &str) -> bool {
    let binds = |pat: &[TT]| pattern_binders(pat).iter().any(|n| n == name);
    let block_at = |from: usize| {
        (from..toks.len())
            .find(|&k| group(toks.get(k), Delimiter::Brace).is_some())
            .unwrap_or(toks.len())
    };
    let mut i = 0;
    while i < toks.len() {
        let prev = if i > 0 { toks.get(i - 1) } else { None };
        match &toks[i] {
            TT::Ident(id) => {
                let s = id.to_string();
                if locus_macro_at(toks, i).is_some() {
                    i += 3;
                    continue;
                }
                match s.as_str() {
                    "let" => {
                        let condition =
                            ident(prev, "if") || ident(prev, "while") || punct(prev, '&');
                        let mut j = i + 1;
                        while j < toks.len()
                            && !single_eq(toks, j)
                            && !punct(toks.get(j), ';')
                            && (condition || !single_colon(toks, j))
                        {
                            j += 1;
                        }
                        let pat = &toks[i + 1..j];
                        if condition {
                            // `if let PAT = EXPR { .. }`: bound in the block.
                            let b = block_at(j);
                            if runtime_mention(&toks[j..b], name) {
                                return true;
                            }
                            i = if binds(pat) { b + 1 } else { b };
                            continue;
                        }
                        let end = (j..toks.len())
                            .find(|&k| punct(toks.get(k), ';'))
                            .unwrap_or(toks.len());
                        // The annotation is skipped: a local can stand in a
                        // type only inside a proposition.
                        let init = (j..end)
                            .find(|&k| single_eq(toks, k))
                            .map_or(end, |k| k + 1);
                        if runtime_mention(&toks[init..end], name) {
                            return true;
                        }
                        if binds(pat) {
                            // Shadowed to the end of this block.
                            return false;
                        }
                        i = end + 1;
                        continue;
                    }
                    "for" => {
                        let in_at = (i..toks.len())
                            .find(|&k| ident(toks.get(k), "in"))
                            .unwrap_or(toks.len());
                        let b = block_at(in_at);
                        if runtime_mention(&toks[in_at.min(b)..b], name) {
                            return true;
                        }
                        i = if binds(&toks[i + 1..in_at]) { b + 1 } else { b };
                        continue;
                    }
                    "match" => {
                        let b = block_at(i + 1);
                        if runtime_mention(&toks[i + 1..b], name) {
                            return true;
                        }
                        if let Some(TT::Group(arms)) = toks.get(b) {
                            let arms = collect(arms.stream());
                            for arm in split_arms(&arms) {
                                if !binds(arm.pat)
                                    && (runtime_mention(arm.guard, name)
                                        || runtime_mention(arm.body, name))
                                {
                                    return true;
                                }
                            }
                        }
                        i = b + 1;
                        continue;
                    }
                    "fn" => {
                        // A nested item cannot name a local.
                        i = block_at(i) + 1;
                        continue;
                    }
                    _ => {}
                }
                if s == name {
                    let field = punct(prev, '.');
                    let label = single_colon(toks, i + 1);
                    let path = punct(prev, ':') || (punct(toks.get(i + 1), ':') && !label);
                    let ghost_statement =
                        single_eq(toks, i + 1) && locus_macro_at(toks, i + 2).is_some();
                    if !(field || label || path || ghost_statement) {
                        return true;
                    }
                }
                i += 1;
            }
            TT::Punct(p) if p.as_char() == '|' && closure_start(prev) => {
                if joint(Some(&toks[i])) && punct(toks.get(i + 1), '|') {
                    i += 2;
                    continue;
                }
                let close = (i + 1..toks.len())
                    .find(|&k| punct(toks.get(k), '|'))
                    .unwrap_or(toks.len());
                let shadowed = split_top(&toks[i + 1..close], ',').iter().any(|(part, _)| {
                    let c = (0..part.len())
                        .find(|&c| single_colon(part, c))
                        .unwrap_or(part.len());
                    binds(&part[..c])
                });
                i = close + 1;
                if shadowed {
                    // Skip the closure's body.
                    if thin_arrow(toks, i) {
                        i = block_at(i) + 1;
                    } else if group(toks.get(i), Delimiter::Brace).is_some() {
                        i += 1;
                    } else {
                        while i < toks.len() && !punct(toks.get(i), ',') && !punct(toks.get(i), ';')
                        {
                            i += 1;
                        }
                    }
                }
            }
            TT::Punct(p) if p.as_char() == '@' => {
                // A proposition in a type inside an expression, as in a
                // turbofish: logical code.
                i += if any_group(toks.get(i + 1)).is_some() {
                    2
                } else {
                    1
                };
            }
            TT::Group(g) => {
                // An inner scope: a binding of the name there ends with it.
                if runtime_mention(&collect(g.stream()), name) {
                    return true;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    false
}

/// `[mut] name`, a pattern that binds one local.
fn simple_binding(pat: &[TT]) -> Option<String> {
    match pat {
        [TT::Ident(n)] if lowercase(&n.to_string()) && n != "_" => Some(n.to_string()),
        [TT::Ident(m), TT::Ident(n)] if m == "mut" => Some(n.to_string()),
        _ => None,
    }
}

fn is_logic_invocation(toks: &[TT]) -> bool {
    toks.len() == 3 && locus_macro_at(toks, 0) == Some("logic")
}

/// `let PAT: TYPE = INIT;` (without the `;`, given separately), and the rest
/// of its block.
fn lower_let(toks: &[TT], semi: Option<&TT>, rest: &[TT], cx: &Cx, sc: &mut Scope) -> TokenStream {
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
    let locus_macro = if init.len() == 3 {
        locus_macro_at(init, 0)
    } else {
        None
    };
    let is_hole = init.len() == 1 && ident(init.first(), "_");
    // A binding shown to be ghost, from the syntax alone.
    let ghost = punct(ty.first(), '@')
        || is_logical_type(ty, cx, &sc.logical_params)
        || locus_macro.is_some()
        || is_hole;
    // The draft's rule. A `logic! { .. }` with no annotation has a type that
    // `locus!` cannot write. If only logical code mentions the local, it
    // holds an untyped marker; otherwise it is one binding whose type rustc
    // infers from runtime code.
    let untyped = |pat: &[TT]| match simple_binding(pat) {
        Some(name) if !cx.all_typed => (!runtime_mention(rest, &name)).then_some(name),
        _ => None,
    };
    let mut ghost_names = Vec::new();
    let mut untyped_names = Vec::new();
    if ghost {
        // Lowering marks ghost bindings for the runtime lints.
        out.extend(parse(
            "#[allow(unused_variables, unused_mut)]",
            toks[0].span(),
        ));
        ghost_names = pattern_binders(pat);
    }
    copy(&mut out, &toks[..pat_end]);
    if !ty.is_empty() {
        out.extend([toks[pat_end].clone()]);
        out.extend(lower_type(ty));
    }
    if single_eq(toks, i) {
        out.extend([toks[i].clone()]);
        // `let (n, before) = (v.len(), logic! { v.len() });`: a tuple
        // pattern over a tuple of the same length, component by component.
        let tuple = match (pat, init) {
            ([TT::Group(p)], [TT::Group(e)])
                if ty.is_empty()
                    && p.delimiter() == Delimiter::Parenthesis
                    && e.delimiter() == Delimiter::Parenthesis =>
            {
                let (p, e) = (collect(p.stream()), collect(e.stream()));
                let (ps, es) = (split_top(&p, ','), split_top(&e, ','));
                (ps.len() == es.len()
                    && ps.len() > 1
                    && es.iter().any(|(e, _)| is_logic_invocation(e)))
                .then(|| {
                    let mut s = TokenStream::new();
                    for ((p, _), (e, sep)) in ps.iter().zip(&es) {
                        if is_logic_invocation(e) {
                            let name = untyped(p);
                            s.extend(lower_logic(
                                kind_of("logic", name.is_some()),
                                any_group(e.get(2)).unwrap(),
                                e[0].span(),
                                cx,
                                sc,
                            ));
                            ghost_names.extend(pattern_binders(p));
                            untyped_names.extend(name);
                        } else {
                            s.extend(lower_seq(e, cx, sc));
                        }
                        s.extend(sep.cloned());
                    }
                    s
                })
            }
            _ => None,
        };
        if let (Some(s), [TT::Group(e)]) = (tuple, init) {
            out.extend([regroup(e, s)]);
        } else if let Some(name) = locus_macro {
            let local = if name == "logic" && ty.is_empty() {
                untyped(pat)
            } else {
                None
            };
            out.extend(lower_logic(
                kind_of(name, local.is_some()),
                any_group(init.get(2)).unwrap(),
                init[0].span(),
                cx,
                sc,
            ));
            untyped_names.extend(local);
        } else if is_hole {
            out.extend(hole(init[0].span()));
        } else {
            out.extend(lower_seq(init, cx, sc));
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
        let mut g = Group::new(Delimiter::Parenthesis, borrows(&places, &sc.ghosts));
        g.set_span(span);
        out.extend([TT::Group(g)]);
        out.extend(parse(";", span));
    }
    // The new bindings are in scope from here, and shadow older ones.
    sc.bind(&pattern_binders(pat), false);
    sc.ghosts.extend(ghost_names);
    sc.untyped.extend(untyped_names);
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
/// wraps: `::locus::KIND!([&places] [values] [names] code)`.
fn lower_logic(kind: &str, g: &Group, span: Span, cx: &Cx, sc: &Scope) -> TokenStream {
    let mut bound = Vec::new();
    let mut places = Vec::new();
    find_places(g.stream(), &mut bound, &mut places);
    // A local called in logical code is not an item.
    bound.extend(sc.locals.iter().cloned());
    let mut probes = Probes::default();
    find_items(g.stream(), cx, &bound, false, &mut probes);
    let mut args = TokenStream::new();
    let mut p = Group::new(Delimiter::Bracket, borrows(&places, &sc.ghosts));
    p.set_span(span);
    args.extend([TT::Group(p)]);
    args.extend(probes.groups(span));
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
                    || name == "_"
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

/// What logical code names, for rustc to resolve in the checking expansion.
#[derive(Default)]
struct Probes {
    /// Expressions: `T::__logic_le`, `Tree::Node`.
    values: Vec<TokenStream>,
    /// Paths for `use path as _;`: `is_max`, `geometry::area`.
    names: Vec<TokenStream>,
}

impl Probes {
    /// `[values] [names]`, without repeats.
    fn groups(&self, span: Span) -> TokenStream {
        let mut out = TokenStream::new();
        for list in [&self.values, &self.names] {
            let mut seen = Vec::new();
            let mut inner = TokenStream::new();
            for item in list {
                let key = item.to_string();
                if seen.contains(&key) {
                    continue;
                }
                seen.push(key);
                if !inner.is_empty() {
                    inner.extend([TT::Punct(Punct::new(',', Spacing::Alone))]);
                }
                inner.extend(item.clone());
            }
            let mut g = Group::new(Delimiter::Bracket, inner);
            g.set_span(span);
            out.extend([TT::Group(g)]);
        }
        out
    }
}

/// The names that logical code calls.
///
/// - `double(..)`, `geometry::area(..)`: a function, probed by name alone.
///   As a value, a generic one would need its type arguments, and `locus!`
///   has no marker to write for them now that each logical type has its own.
/// - `T::le(..)`, `Self::le(..)`, `Rational::le(..)`: a logic function of a
///   type or of a trait, probed as the value `T::__logic_le`. The type
///   arguments of a generic logical type that `locus!` knows are written as
///   the untyped marker.
/// - `<T as OrderSpec>::le(..)`: the same, through the hidden trait that
///   holds the stubs.
/// - `Tree::Node(..)`: a constructor, probed as a value. Not in the body of
///   a logic function, where it may be a pattern.
///
/// GUESS: a path whose segment before the last starts with a capital names
/// a type, a type parameter or a trait, and any other names a module. Which
/// it is needs name resolution. `OrderSpec::le(..)`, a trait where a type
/// would stand, cannot be told from `Rational::le(..)`.
fn find_items(ts: TokenStream, cx: &Cx, bound: &[String], in_logic_fn: bool, out: &mut Probes) {
    const NOT_ITEMS: &[&str] = &["snap", "final", "forall", "exists", "model", "old"];
    let mut toks = Vec::new();
    flatten(ts, &mut toks);
    for k in 0..toks.len() {
        if let TT::Group(g) = &toks[k] {
            find_items(g.stream(), cx, bound, in_logic_fn, out);
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
        let mangle = |t: &TT| TT::Ident(Ident::new(&mangled(&name), t.span()));
        let colons =
            |at: usize| at >= 2 && punct(toks.get(at - 1), ':') && punct(toks.get(at - 2), ':');
        // `<T as Trait>::name(`
        if colons(k) && k >= 3 && punct(toks.get(k - 3), '>') {
            let mut depth = 0;
            let mut open = None;
            for b in (0..=k - 3).rev() {
                if punct(toks.get(b), '>') {
                    depth += 1;
                } else if punct(toks.get(b), '<') {
                    depth -= 1;
                    if depth == 0 {
                        open = Some(b);
                        break;
                    }
                }
            }
            let Some(open) = open else { continue };
            let inner = &toks[open + 1..k - 3];
            let mut item = TokenStream::new();
            item.extend([toks[open].clone()]);
            // The trait's name: the last identifier after `as`, outside `<..>`.
            let mut depth = 0;
            let mut last = None;
            let as_at = inner.iter().position(|t| ident(Some(t), "as"));
            for (b, t) in inner.iter().enumerate() {
                if punct(Some(t), '<') {
                    depth += 1;
                } else if punct(Some(t), '>') {
                    depth -= 1;
                } else if depth == 0
                    && as_at.is_some_and(|a| b > a)
                    && ident_name(Some(t)).is_some()
                {
                    last = Some(b);
                }
            }
            for (b, t) in inner.iter().enumerate() {
                match t {
                    TT::Ident(id) if Some(b) == last && !cx.members_in_trait => {
                        item.extend([TT::Ident(Ident::new(
                            &companion(&id.to_string()),
                            id.span(),
                        ))]);
                    }
                    t => item.extend([t.clone()]),
                }
            }
            copy(&mut item, &toks[k - 3..k]);
            item.extend([mangle(&toks[k])]);
            out.values.push(item);
            continue;
        }
        let mut start = k;
        while colons(start) && start >= 3 && ident_name(toks.get(start - 3)).is_some() {
            start -= 3;
        }
        if start == k {
            if lowercase(&name) && !bound.contains(&name) && !NOT_ITEMS.contains(&name.as_str()) {
                out.names.push(TokenStream::from(toks[k].clone()));
            }
            continue;
        }
        let first = toks[start].to_string();
        let before_last = toks[k - 3].to_string();
        if lowercase(&before_last) {
            // `geometry::area(..)`: a function in a module.
            if lowercase(&name) {
                out.names.push(toks[start..=k].iter().cloned().collect());
            }
            continue;
        }
        if !lowercase(&name) && in_logic_fn {
            continue;
        }
        let mut item = TokenStream::from(toks[start].clone());
        if let Some(n) = cx.logical(&first).filter(|n| *n > 0) {
            // The type arguments of a generic logical type. There is no
            // marker that stands for every logical type, so the untyped one
            // is written; it satisfies no bound.
            let list = vec!["::locus::Untyped"; n].join(", ");
            item.extend(parse(&format!("::<{list}>"), toks[start].span()));
        }
        copy(&mut item, &toks[start + 1..k]);
        item.extend([if lowercase(&name) {
            mangle(&toks[k])
        } else {
            toks[k].clone()
        }]);
        out.values.push(item);
    }
}

/// `prove!` and friends expanded by rustc: observe each place precisely
/// (the toy has no table of ghost roots), then give the marker. A `logic!`
/// here gives a marker of the type rustc infers.
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
    body.extend(parse(marker, Span::call_site()));
    TokenStream::from(TT::Group(Group::new(Delimiter::Brace, body)))
}
