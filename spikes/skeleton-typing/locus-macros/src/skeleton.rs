//! The typing skeleton: logical code translated, from its tokens alone, into
//! a Rust expression over the stubs of `::locus::sk` and the stubs of the
//! program's logic functions. `locus!` puts it in a closure that is never
//! called, so rustc types it, resolves its names and its traits, and gives
//! a ghost `let` its type.
//!
//! The translation is syntactic. It never knows a type, and it never knows
//! whether a name is a runtime local, a ghost local, or a binder of the
//! logical code itself. Everything that depends on a type is left to rustc,
//! through a stub whose result is a projection or is inferred:
//!
//! | Logical code | Skeleton |
//! |---|---|
//! | a place read as a value: `x`, `p.a`, `*r` | `sk::model_of(&x)` |
//! | a place as an argument, a field's initializer, a logic function's result | `sk::arg(&x)` |
//! | `snap(x)`, `final!(r)` | `sk::snap(&x)`, the place `*sk::final_of(&r)` |
//! | `f(a)`, `T::f(a)`, `<T as Tr>::f(a)`, `x.f(a)` | `f(..)`, `T::__logic_f(..)`, `<T as Tr>::__logic_f(..)`, `model_of(&x).__logic_f(..)` |
//! | `a + b`, `a < b`, `a == b`, `a === b`, `a && b`, `!a`, `a ==> b` | `sk::add(a, b)`, `sk::lt(a, b)`, .. |
//! | `3` | `sk::lit(3)`; beside another operand, `sk::lit_like(&that, 3)` |
//! | `if c { a } else { b }` | `if sk::cond(c) { a } else { b }` |
//! | `match t { Tree::Node(l, _, r) => e }` | `match sk::view(t) { __locus_view_Tree::Node(l, _, r) => e }` |
//! | `forall \|x: T\| p`, `\|x: T\| proof` | `sk::forall1(\|x: T\| p)`, `sk::lambda1(\|x: T\| proof)` |
//! | `_`, `prove!(p)` | `sk::hole()`, `sk::proved(\|\| p)` |
//!
//! What it cannot translate is an `Err` that names the construct, and the
//! caller falls back to the first mode's lowering.
use crate::*;

pub(crate) struct Opts {
    /// A trait's stubs are items of the lowered trait (and not of a hidden
    /// trait beside it): decides how `<T as Trait>::f` is written.
    pub members_in_trait: bool,
    /// Ghost locals in scope that hold the untyped marker: logical code that
    /// reads one has no skeleton.
    pub untyped: Vec<String>,
    /// The code is all of a `logic!` that initializes a `let` with no
    /// annotation, and `LOCUS_SPIKE_LITERAL_INT=1`: if it is one integer
    /// literal, nothing is expected of it, and Locus's rule makes it an `Int`.
    pub lone_literal_is_int: bool,
}

/// How a place is read where it stands.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Cx {
    /// As a value: its model.
    Value,
    /// Where a type is wanted that rustc knows: an argument, a field's
    /// initializer, the result of a logic function.
    Arg,
}

type R<T> = Result<T, String>;

enum Pl {
    /// A local, a parameter, `self`, or a static: one or more path tokens.
    Local(Vec<TT>),
    Field(Box<Pl>, TT, TT),
    Deref(Box<Pl>, TT),
    Final(Box<Pl>, Span),
}

enum Ex {
    Lit(TT),
    Bool(TT),
    Hole(Span),
    Place(Pl),
    Path(Vec<TT>),
    Call(Vec<TT>, Group, Vec<Ex>),
    Method(Box<Ex>, Vec<TT>, Group, Vec<Ex>),
    Field(Box<Ex>, TT, TT),
    Index(Box<Ex>, Box<Ex>, Span),
    Unary(char, Span, Box<Ex>),
    Binary(&'static str, Span, Box<Ex>, Box<Ex>),
    If(Span, Box<Ex>, Block, Box<Ex>),
    Match(Span, Box<Ex>, Group, Vec<MatchArm>),
    Struct(Vec<TT>, Group, Vec<(TT, Ex)>),
    Tuple(Group, Vec<Ex>),
    Paren(Group, Box<Ex>),
    Block(Block),
    Lambda(Span, Vec<TT>, usize, Box<Ex>),
    Quant(&'static str, Span, Vec<TT>, usize, Box<Ex>),
    Snap(Span, Pl),
    Prove(Span, Box<Ex>),
}

struct Block {
    span: Span,
    /// `let PAT [: TYPE] = INIT;`
    lets: Vec<(Vec<TT>, Vec<TT>, Ex)>,
    tail: Box<Ex>,
}

struct MatchArm {
    pat: TokenStream,
    /// Names that the proof part of a pattern binds: each is a proof.
    proofs: Vec<TT>,
    guard: Option<Ex>,
    body: Ex,
}

/// The skeleton of a block of logical code, without its braces.
pub(crate) fn translate(ts: TokenStream, tail: Cx, opts: &Opts) -> R<TokenStream> {
    let mut toks = Vec::new();
    flatten(ts, &mut toks);
    let block = block_contents(&toks, Span::call_site(), opts)?;
    let e = Emit { opts };
    if let (true, true, Some(lit)) = (
        opts.lone_literal_is_int,
        block.lets.is_empty(),
        literal(&block.tail),
    ) {
        let mut out = sk("lit", lit.span());
        out.extend(parse("::<::locus::Int>", lit.span()));
        let mut g = Group::new(Delimiter::Parenthesis, TokenStream::from(lit.clone()));
        g.set_span(lit.span());
        out.extend([TT::Group(g)]);
        return Ok(out);
    }
    e.block_inner(&block, tail)
}

// ------------------------------------------------------------------ parsing

const OPERATORS: &[&str] = &[
    "<==>", "===", "!==", "==>", "==", "!=", "<=", ">=", "&&", "||", "<", ">", "+", "-", "*", "/",
    "%",
];

fn level(op: &str) -> u8 {
    match op {
        "<==>" => 1,
        "==>" => 2,
        "||" => 3,
        "&&" => 4,
        "==" | "!=" | "<" | "<=" | ">" | ">=" | "===" | "!==" => 5,
        "+" | "-" => 6,
        _ => 7,
    }
}

fn screaming(name: &str) -> bool {
    name.len() > 1
        && name
            .chars()
            .all(|c| c.is_uppercase() || c.is_ascii_digit() || c == '_')
        && name.chars().any(|c| c.is_uppercase())
}

struct Parser<'a> {
    t: &'a [TT],
    i: usize,
    opts: &'a Opts,
}

fn block_contents(toks: &[TT], span: Span, opts: &Opts) -> R<Block> {
    let mut lets = Vec::new();
    let mut start = 0;
    loop {
        let semi = (start..toks.len()).find(|&k| punct(toks.get(k), ';'));
        let Some(semi) = semi else { break };
        let stmt = &toks[start..semi];
        if !ident(stmt.first(), "let") {
            return Err("a statement that is not a `let`".into());
        }
        let mut k = 1;
        while k < stmt.len() && !single_colon(stmt, k) && !single_eq(stmt, k) {
            k += 1;
        }
        let pat = stmt[1..k].to_vec();
        let mut ty = Vec::new();
        if single_colon(stmt, k) {
            let s = k + 1;
            while k < stmt.len() && !single_eq(stmt, k) {
                k += 1;
            }
            ty = stmt[s..k].to_vec();
        }
        if !single_eq(stmt, k) {
            return Err("a `let` with no initializer".into());
        }
        let init = whole(&stmt[k + 1..], opts)?;
        lets.push((pat, ty, init));
        start = semi + 1;
    }
    if start >= toks.len() {
        return Err("a block with no value".into());
    }
    let tail = whole(&toks[start..], opts)?;
    Ok(Block {
        span,
        lets,
        tail: Box::new(tail),
    })
}

/// One expression that is all of `toks`.
fn whole(toks: &[TT], opts: &Opts) -> R<Ex> {
    let mut flat = Vec::new();
    flatten(toks.iter().cloned().collect(), &mut flat);
    let mut p = Parser {
        t: &flat,
        i: 0,
        opts,
    };
    let e = p.expr(false)?;
    match p.t.get(p.i) {
        None => Ok(e),
        Some(TT::Ident(id)) if id == "as" => Err("a cast with `as`".into()),
        Some(t) => Err(format!("the token `{t}`")),
    }
}

fn list(toks: &[TT], opts: &Opts) -> R<Vec<Ex>> {
    let mut out = Vec::new();
    for (part, _) in split_commas(toks) {
        out.push(whole(part, opts)?);
    }
    Ok(out)
}

/// Splits at top-level commas. Unlike `split_top`, `<` and `>` are
/// operators here, not brackets.
fn split_commas(toks: &[TT]) -> Vec<(&[TT], bool)> {
    let mut parts = Vec::new();
    let mut start = 0;
    for i in 0..toks.len() {
        if punct(toks.get(i), ',') {
            parts.push((&toks[start..i], true));
            start = i + 1;
        }
    }
    if start < toks.len() {
        parts.push((&toks[start..], false));
    }
    parts
}

impl Parser<'_> {
    fn peek(&self) -> Option<&TT> {
        self.t.get(self.i)
    }

    /// The operator at the cursor, and its length in tokens.
    fn operator(&self) -> Option<(&'static str, usize)> {
        let mut text = String::new();
        let mut k = self.i;
        while text.len() < 4 {
            match self.t.get(k) {
                Some(TT::Punct(p)) => {
                    text.push(p.as_char());
                    k += 1;
                    if p.spacing() != Spacing::Joint {
                        break;
                    }
                }
                _ => break,
            }
        }
        (1..=text.len()).rev().find_map(|n| {
            OPERATORS
                .iter()
                .find(|op| **op == &text[..n])
                .map(|op| (*op, n))
        })
    }

    fn expr(&mut self, no_struct: bool) -> R<Ex> {
        self.binary(0, no_struct)
    }

    fn binary(&mut self, min: u8, no_struct: bool) -> R<Ex> {
        let mut left = self.unary(no_struct)?;
        while let Some((op, n)) = self.operator() {
            let l = level(op);
            if l < min {
                break;
            }
            let span = self.t[self.i].span();
            self.i += n;
            // `==>` associates to the right; a comparison does not chain.
            let next = if op == "==>" { l } else { l + 1 };
            let right = self.binary(next, no_struct)?;
            if l == 5 || l == 1 {
                if let Some((again, _)) = self.operator() {
                    if level(again) == l {
                        return Err(format!("a chain of `{op}` and `{again}`"));
                    }
                }
            }
            left = Ex::Binary(op, span, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn unary(&mut self, no_struct: bool) -> R<Ex> {
        match self.peek() {
            Some(TT::Punct(p)) if "!-*&".contains(p.as_char()) => {
                let (c, span, tok) = (p.as_char(), p.span(), self.t[self.i].clone());
                if c == '&' {
                    return Err("a borrow `&`".into());
                }
                self.i += 1;
                let inner = self.unary(no_struct)?;
                if c == '*' {
                    return match inner {
                        Ex::Place(p) => Ok(Ex::Place(Pl::Deref(Box::new(p), tok))),
                        _ => Err("`*` on what is not a place".into()),
                    };
                }
                Ok(Ex::Unary(c, span, Box::new(inner)))
            }
            Some(TT::Punct(p)) if p.as_char() == '|' => self.closure(None),
            _ => {
                let e = self.primary(no_struct)?;
                self.postfix(e)
            }
        }
    }

    /// `|x: T, y: U| body`, after an optional `forall` or `exists`.
    fn closure(&mut self, quantifier: Option<&'static str>) -> R<Ex> {
        let span = self.t[self.i].span();
        if joint(self.peek()) && punct(self.t.get(self.i + 1), '|') {
            return Err("a closure with no parameters".into());
        }
        let close = (self.i + 1..self.t.len())
            .find(|&k| punct(self.t.get(k), '|'))
            .ok_or("a closure with no closing `|`")?;
        let params = &self.t[self.i + 1..close];
        let mut lowered = TokenStream::new();
        let mut count = 0;
        for (part, sep) in split_top(params, ',') {
            let Some(c) = (0..part.len()).find(|&c| single_colon(part, c)) else {
                return Err("a closure parameter with no type".into());
            };
            copy(&mut lowered, &part[..=c]);
            lowered.extend(lower_type(&part[c + 1..]));
            lowered.extend(sep.cloned());
            count += 1;
        }
        if count == 0 || count > 3 {
            return Err(format!("a closure with {count} parameters"));
        }
        self.i = close + 1;
        let body = self.expr(false)?;
        let params = collect(lowered);
        Ok(match quantifier {
            Some(q) => Ex::Quant(q, span, params, count, Box::new(body)),
            None => Ex::Lambda(span, params, count, Box::new(body)),
        })
    }

    fn postfix(&mut self, mut e: Ex) -> R<Ex> {
        loop {
            match self.peek() {
                Some(TT::Punct(p)) if p.as_char() == '.' && !joint(self.peek()) => {
                    let dot = self.t[self.i].clone();
                    match self.t.get(self.i + 1) {
                        Some(TT::Ident(name)) => {
                            let name_tok = self.t[self.i + 1].clone();
                            // `.name(args)` or `.name::<T>(args)`
                            let mut k = self.i + 2;
                            let mut fish = Vec::new();
                            if punct(self.t.get(k), ':') && punct(self.t.get(k + 1), ':') {
                                let (g, next) = generics_at(self.t, k + 2);
                                fish = self.t[k..k + 2].to_vec();
                                fish.extend(lower_type(g));
                                k = next;
                            }
                            if let Some(args) = group(self.t.get(k), Delimiter::Parenthesis) {
                                let mut callee = vec![
                                    dot,
                                    TT::Ident(Ident::new(&mangled(&name.to_string()), name.span())),
                                ];
                                callee.extend(fish);
                                let list = list(&collect(args.stream()), self.opts)?;
                                e = Ex::Method(Box::new(e), callee, args.clone(), list);
                                self.i = k + 1;
                            } else {
                                e = match e {
                                    Ex::Place(p) => {
                                        Ex::Place(Pl::Field(Box::new(p), dot, name_tok))
                                    }
                                    other => Ex::Field(Box::new(other), dot, name_tok),
                                };
                                self.i += 2;
                            }
                        }
                        Some(TT::Literal(lit)) => {
                            // `pair.0`, and `pair.0.1`, which is one token `0.1`.
                            for part in lit.to_string().split('.') {
                                if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
                                    return Err(format!("the field `{lit}`"));
                                }
                                let mut index: TT = TT::Literal(
                                    proc_macro2::Literal::usize_unsuffixed(part.parse().unwrap()),
                                );
                                index.set_span(lit.span());
                                e = match e {
                                    Ex::Place(p) => {
                                        Ex::Place(Pl::Field(Box::new(p), dot.clone(), index))
                                    }
                                    other => Ex::Field(Box::new(other), dot.clone(), index),
                                };
                            }
                            self.i += 2;
                        }
                        _ => return Err("a `.` with no field after it".into()),
                    }
                }
                Some(TT::Group(g)) if g.delimiter() == Delimiter::Bracket => {
                    let index = whole(&collect(g.stream()), self.opts)?;
                    e = Ex::Index(Box::new(e), Box::new(index), g.span());
                    self.i += 1;
                }
                Some(TT::Group(g)) if g.delimiter() == Delimiter::Parenthesis => {
                    return Err("a call of what is not a path".into());
                }
                Some(TT::Punct(p)) if p.as_char() == '?' => {
                    return Err("the operator `?`".into());
                }
                _ => return Ok(e),
            }
        }
    }

    fn primary(&mut self, no_struct: bool) -> R<Ex> {
        let Some(t) = self.peek().cloned() else {
            return Err("an expression that ends early".into());
        };
        match &t {
            TT::Literal(lit) => {
                let text = lit.to_string();
                if !text.chars().all(|c| c.is_ascii_digit() || c == '_') {
                    return Err(format!("the literal `{text}`"));
                }
                self.i += 1;
                Ok(Ex::Lit(t))
            }
            TT::Group(g) => {
                self.i += 1;
                let inner = collect(g.stream());
                match g.delimiter() {
                    Delimiter::Parenthesis => {
                        let parts = split_commas(&inner);
                        if parts.len() == 1 && !parts[0].1 {
                            let e = whole(parts[0].0, self.opts)?;
                            Ok(match e {
                                // `(*r).len`: still a place.
                                Ex::Place(p) => Ex::Place(p),
                                e => Ex::Paren(g.clone(), Box::new(e)),
                            })
                        } else if parts.is_empty() {
                            Err("the unit value `()`".into())
                        } else {
                            Ok(Ex::Tuple(g.clone(), list(&inner, self.opts)?))
                        }
                    }
                    Delimiter::Brace => Ok(Ex::Block(block_contents(&inner, g.span(), self.opts)?)),
                    Delimiter::Bracket => Err("an array `[..]`".into()),
                    Delimiter::None => whole(&inner, self.opts),
                }
            }
            TT::Punct(p) if p.as_char() == '<' => self.qualified(),
            TT::Punct(p) => Err(format!("the token `{}`", p.as_char())),
            TT::Ident(id) => {
                let name = id.to_string();
                match name.as_str() {
                    "true" | "false" => {
                        self.i += 1;
                        Ok(Ex::Bool(t))
                    }
                    "_" => {
                        self.i += 1;
                        Ok(Ex::Hole(id.span()))
                    }
                    "if" => self.if_(),
                    "match" => self.match_(),
                    "forall" | "exists" if punct(self.t.get(self.i + 1), '|') => {
                        self.i += 1;
                        self.closure(Some(if name == "forall" { "forall" } else { "exists" }))
                    }
                    "let" | "loop" | "while" | "for" | "return" | "break" | "continue"
                    | "unsafe" | "async" | "move" | "const" | "static" | "fn" | "struct"
                    | "impl" | "use" | "mut" | "ref" | "dyn" | "await" => {
                        Err(format!("the keyword `{name}`"))
                    }
                    _ => self.path(no_struct),
                }
            }
        }
    }

    fn if_(&mut self) -> R<Ex> {
        let span = self.t[self.i].span();
        self.i += 1;
        if ident(self.peek(), "let") {
            return Err("an `if let`".into());
        }
        let cond = self.expr(true)?;
        let Some(then) = group(self.peek(), Delimiter::Brace).cloned() else {
            return Err("an `if` with no block".into());
        };
        self.i += 1;
        if !ident(self.peek(), "else") {
            return Err("an `if` with no `else`".into());
        }
        self.i += 1;
        let then = block_contents(&collect(then.stream()), then.span(), self.opts)?;
        let els = if ident(self.peek(), "if") {
            self.if_()?
        } else if let Some(g) = group(self.peek(), Delimiter::Brace).cloned() {
            self.i += 1;
            Ex::Block(block_contents(&collect(g.stream()), g.span(), self.opts)?)
        } else {
            return Err("an `else` with no block".into());
        };
        Ok(Ex::If(span, Box::new(cond), then, Box::new(els)))
    }

    fn match_(&mut self) -> R<Ex> {
        let span = self.t[self.i].span();
        self.i += 1;
        let scrutinee = self.expr(true)?;
        let Some(body) = group(self.peek(), Delimiter::Brace).cloned() else {
            return Err("a `match` with no arms".into());
        };
        self.i += 1;
        let inner = collect(body.stream());
        let mut arms = Vec::new();
        for arm in split_arms(&inner) {
            // `PATTERN @ PROOF-PATTERN`: what stands after `@` binds proofs.
            let at = arm.pat.iter().position(|t| punct(Some(t), '@'));
            let (pat, proof) = match at {
                Some(at) => (&arm.pat[..at], &arm.pat[at + 1..]),
                None => (arm.pat, &arm.pat[..0]),
            };
            let guard = match arm.guard {
                [] => None,
                [_if, rest @ ..] => Some(whole(rest, self.opts)?),
            };
            let body = match arm.body {
                [] => return Err("a `match` arm with no body".into()),
                body => whole(body, self.opts)?,
            };
            arms.push(MatchArm {
                pat: view_pattern(pat, 0)?,
                proofs: proof_binders(proof),
                guard,
                body,
            });
        }
        Ok(Ex::Match(span, Box::new(scrutinee), body, arms))
    }

    /// `<T as Trait>::name(args)`.
    fn qualified(&mut self) -> R<Ex> {
        let (angle, next) = generics_at(self.t, self.i);
        if angle.len() < 2 || !punct(self.t.get(next), ':') || !punct(self.t.get(next + 1), ':') {
            return Err("the token `<`".into());
        }
        let Some(TT::Ident(name)) = self.t.get(next + 2) else {
            return Err("a qualified path with no name".into());
        };
        let Some(args) = group(self.t.get(next + 3), Delimiter::Parenthesis).cloned() else {
            return Err("a qualified path that is not called".into());
        };
        // The trait's name is the last identifier after `as`, outside `<..>`.
        let inner = &angle[1..angle.len() - 1];
        let as_at = inner.iter().position(|t| ident(Some(t), "as"));
        let (mut depth, mut last) = (0, None);
        for (b, t) in inner.iter().enumerate() {
            if punct(Some(t), '<') {
                depth += 1;
            } else if punct(Some(t), '>') {
                depth -= 1;
            } else if depth == 0 && as_at.is_some_and(|a| b > a) && ident_name(Some(t)).is_some() {
                last = Some(b);
            }
        }
        let mut callee = vec![angle[0].clone()];
        for (b, t) in inner.iter().enumerate() {
            match t {
                TT::Ident(id) if Some(b) == last && !self.opts.members_in_trait => {
                    callee.push(TT::Ident(Ident::new(&companion(&id.to_string()), id.span())));
                }
                t => callee.push(t.clone()),
            }
        }
        callee.push(angle[angle.len() - 1].clone());
        callee.extend(self.t[next..next + 2].iter().cloned());
        callee.push(if lowercase(&name.to_string()) {
            TT::Ident(Ident::new(&mangled(&name.to_string()), name.span()))
        } else {
            TT::Ident(name.clone())
        });
        self.i = next + 4;
        let list = list(&collect(args.stream()), self.opts)?;
        Ok(Ex::Call(callee, args, list))
    }

    /// A path: a place, a call, a macro, a struct literal, or a value.
    fn path(&mut self, no_struct: bool) -> R<Ex> {
        let start = self.i;
        let mut segments: Vec<usize> = vec![self.i];
        self.i += 1;
        loop {
            if punct(self.peek(), ':') && joint(self.peek()) && punct(self.t.get(self.i + 1), ':') {
                if punct(self.t.get(self.i + 2), '<') {
                    let (_, next) = generics_at(self.t, self.i + 2);
                    self.i = next;
                } else if ident_name(self.t.get(self.i + 2)).is_some() {
                    segments.push(self.i + 2);
                    self.i += 3;
                } else {
                    return Err("a path that ends in `::`".into());
                }
            } else {
                break;
            }
        }
        let toks = self.t[start..self.i].to_vec();
        let last_at = *segments.last().unwrap();
        let last = self.t[last_at].to_string();
        let single = segments.len() == 1;
        let before = (!single).then(|| self.t[segments[segments.len() - 2]].to_string());
        let before = before.as_deref();
        // A macro.
        if let (true, Some(g)) = (
            punct(self.peek(), '!'),
            any_group(self.t.get(self.i + 1)).cloned(),
        ) {
            let span = self.t[start].span();
            self.i += 2;
            let inner = collect(g.stream());
            return match (single, last.as_str()) {
                (true, "final") => match whole(&inner, self.opts)? {
                    Ex::Place(p) => Ok(Ex::Place(Pl::Final(Box::new(p), span))),
                    _ => Err("`final!` of what is not a place".into()),
                },
                (true, "prove") => Ok(Ex::Prove(span, Box::new(whole(&inner, self.opts)?))),
                (true, "logic" | "prop") => {
                    Ok(Ex::Block(block_contents(&inner, g.span(), self.opts)?))
                }
                _ => Err(format!("the macro `{last}!`")),
            };
        }
        // A call.
        if let Some(args) = group(self.peek(), Delimiter::Parenthesis).cloned() {
            self.i += 1;
            let inner = collect(args.stream());
            if single && (last == "snap" || last == "model") {
                return match (last.as_str(), whole(&inner, self.opts)?) {
                    ("snap", Ex::Place(p)) => Ok(Ex::Snap(self.t[start].span(), p)),
                    ("model", Ex::Place(p)) => Ok(Ex::Place(p)),
                    _ => Err(format!("`{last}` of what is not a place")),
                };
            }
            if single && last == "old" {
                return Err("`old(..)`".into());
            }
            let mut callee = toks.clone();
            // `T::le(..)`, `Rational::le(..)`, `Self::le(..)`: the stub of a
            // logic function of a type or a trait has a mangled name. GUESS,
            // as the first mode's probes: a capital on the segment before
            // the last names a type, a type parameter or a trait.
            if before.is_some_and(|b| !lowercase(b)) && lowercase(&last) {
                let k = last_at - start;
                callee[k] = TT::Ident(Ident::new(&mangled(&last), self.t[last_at].span()));
            }
            return Ok(Ex::Call(callee, args, list(&inner, self.opts)?));
        }
        // A struct literal.
        if let (false, Some(g)) = (no_struct, group(self.peek(), Delimiter::Brace).cloned()) {
            if !lowercase(&last) {
                self.i += 1;
                let inner = collect(g.stream());
                let mut fields = Vec::new();
                for (part, _) in split_commas(&inner) {
                    match part {
                        [name @ TT::Ident(_)] => {
                            let place = Ex::Place(Pl::Local(vec![name.clone()]));
                            fields.push((name.clone(), place));
                        }
                        [name @ TT::Ident(_), TT::Punct(c), rest @ ..] if c.as_char() == ':' => {
                            fields.push((name.clone(), whole(rest, self.opts)?));
                        }
                        _ => return Err("a struct literal with `..` or an unnamed field".into()),
                    }
                }
                return Ok(Ex::Struct(toks, g, fields));
            }
        }
        // A place: a local, a parameter, `self`, or a static.
        if (single && lowercase(&last)) || screaming(&last) && before.is_none_or(lowercase)
        {
            return Ok(Ex::Place(Pl::Local(toks)));
        }
        Ok(Ex::Path(toks))
    }
}

/// A pattern of a `match` in logical code. The variants of a logical enum
/// `Tree` are those of its view, `__locus_view_Tree`, so a path of two or
/// more segments that ends in a capital is rewritten to name the view.
fn view_pattern(pat: &[TT], depth: usize) -> R<TokenStream> {
    let mut out = TokenStream::new();
    let mut i = 0;
    while i < pat.len() {
        match &pat[i] {
            TT::Group(g) => {
                out.extend([regroup(g, view_pattern(&collect(g.stream()), depth + 1)?)]);
                i += 1;
            }
            TT::Literal(l) => return Err(format!("the literal pattern `{l}`")),
            TT::Ident(id)
                if punct(pat.get(i + 1), ':')
                    && joint(pat.get(i + 1))
                    && punct(pat.get(i + 2), ':') =>
            {
                // A path: find its last two segments.
                let mut k = i;
                while punct(pat.get(k + 1), ':')
                    && punct(pat.get(k + 2), ':')
                    && ident_name(pat.get(k + 3)).is_some()
                {
                    k += 3;
                }
                let last = pat[k].to_string();
                if lowercase(&last) {
                    return Err("a path in a pattern that is not a variant".into());
                }
                if depth > 0 {
                    // The payload of a view's variant is the logical type
                    // itself, which has no variants for rustc.
                    return Err("a variant pattern inside a variant pattern".into());
                }
                let _ = id;
                copy(&mut out, &pat[i..k - 3]);
                let owner = &pat[k - 3];
                out.extend([TT::Ident(Ident::new(
                    &format!("__locus_view_{owner}"),
                    owner.span(),
                ))]);
                copy(&mut out, &pat[k - 2..=k]);
                i = k + 1;
            }
            t => {
                out.extend([t.clone()]);
                i += 1;
            }
        }
    }
    Ok(out)
}

/// The names that the proof part of a pattern binds: lowercase identifiers
/// that are not path segments.
fn proof_binders(pat: &[TT]) -> Vec<TT> {
    let mut out = Vec::new();
    for (i, t) in pat.iter().enumerate() {
        match t {
            TT::Group(g) => out.extend(proof_binders(&collect(g.stream()))),
            TT::Ident(id) => {
                let n = id.to_string();
                let path = punct(pat.get(i + 1), ':') || (i > 0 && punct(pat.get(i - 1), ':'));
                if lowercase(&n) && n != "_" && !path {
                    out.push(t.clone());
                }
            }
            _ => {}
        }
    }
    out
}

// ----------------------------------------------------------------- emitting

struct Emit<'a> {
    opts: &'a Opts,
}

/// `::locus::sk::name`, every token at `span`.
fn sk(name: &str, span: Span) -> TokenStream {
    parse(&format!("::locus::sk::{name}"), span)
}

fn call(name: &str, span: Span, args: TokenStream) -> TokenStream {
    let mut out = sk(name, span);
    let mut g = Group::new(Delimiter::Parenthesis, args);
    g.set_span(span);
    out.extend([TT::Group(g)]);
    out
}

fn comma(span: Span) -> TT {
    let mut p = Punct::new(',', Spacing::Alone);
    p.set_span(span);
    TT::Punct(p)
}

fn braces(span: Span, inner: TokenStream) -> TokenStream {
    let mut g = Group::new(Delimiter::Brace, inner);
    g.set_span(span);
    TokenStream::from(TT::Group(g))
}

fn span_of(e: &Ex) -> Span {
    match e {
        Ex::Lit(t) | Ex::Bool(t) => t.span(),
        Ex::Hole(s) | Ex::Unary(_, s, _) | Ex::Binary(_, s, ..) | Ex::If(s, ..) => *s,
        Ex::Match(s, ..) | Ex::Lambda(s, ..) | Ex::Quant(_, s, ..) | Ex::Snap(s, _) => *s,
        Ex::Prove(s, _) | Ex::Index(_, _, s) => *s,
        Ex::Place(p) => place_span(p),
        Ex::Path(t) | Ex::Call(t, ..) | Ex::Struct(t, ..) => t[0].span(),
        Ex::Method(r, ..) | Ex::Field(r, ..) => span_of(r),
        Ex::Tuple(g, _) | Ex::Paren(g, _) => g.span(),
        Ex::Block(b) => b.span,
    }
}

fn place_span(p: &Pl) -> Span {
    match p {
        Pl::Local(t) => t[0].span(),
        Pl::Field(p, ..) | Pl::Deref(p, _) => place_span(p),
        Pl::Final(_, s) => *s,
    }
}

fn literal(e: &Ex) -> Option<&TT> {
    match e {
        Ex::Lit(t) => Some(t),
        Ex::Paren(_, inner) => literal(inner),
        _ => None,
    }
}

impl Emit<'_> {
    /// The place, as a Rust place expression.
    fn place(&self, p: &Pl) -> R<TokenStream> {
        Ok(match p {
            Pl::Local(t) => {
                if t.len() == 1 && self.opts.untyped.contains(&t[0].to_string()) {
                    return Err(format!(
                        "it reads `{}`, a ghost local that was left untyped",
                        t[0]
                    ));
                }
                t.iter().cloned().collect()
            }
            Pl::Field(base, dot, name) => {
                let mut out = self.place(base)?;
                out.extend([dot.clone(), name.clone()]);
                out
            }
            Pl::Deref(base, star) => {
                let mut inner = TokenStream::from(star.clone());
                inner.extend(self.place(base)?);
                let mut g = Group::new(Delimiter::Parenthesis, inner);
                g.set_span(star.span());
                TokenStream::from(TT::Group(g))
            }
            Pl::Final(base, span) => {
                let mut arg = parse("&", *span);
                arg.extend(self.place(base)?);
                let mut inner = parse("*", *span);
                inner.extend(call("final_of", *span, arg));
                let mut g = Group::new(Delimiter::Parenthesis, inner);
                g.set_span(*span);
                TokenStream::from(TT::Group(g))
            }
        })
    }

    /// `&place`, the `&` at the place's span.
    fn borrow(&self, p: &Pl) -> R<TokenStream> {
        let mut out = parse("&", place_span(p));
        out.extend(self.place(p)?);
        Ok(out)
    }

    fn list(&self, items: &[Ex], cx: Cx, span: Span) -> R<TokenStream> {
        let mut out = TokenStream::new();
        for e in items {
            out.extend(self.expr(e, cx)?);
            out.extend([comma(span)]);
        }
        Ok(out)
    }

    fn block_inner(&self, b: &Block, cx: Cx) -> R<TokenStream> {
        let mut out = TokenStream::new();
        for (pat, ty, init) in &b.lets {
            out.extend(parse("let", b.span));
            copy(&mut out, pat);
            if ty.is_empty() {
                out.extend(parse("=", b.span));
                out.extend(self.expr(init, Cx::Value)?);
            } else {
                out.extend(parse(":", b.span));
                out.extend(lower_type(ty));
                out.extend(parse("=", b.span));
                out.extend(self.expr(init, Cx::Arg)?);
            }
            out.extend(parse(";", b.span));
        }
        out.extend(self.expr(&b.tail, cx)?);
        Ok(out)
    }

    fn expr(&self, e: &Ex, cx: Cx) -> R<TokenStream> {
        Ok(match e {
            Ex::Lit(t) => call("lit", t.span(), TokenStream::from(t.clone())),
            Ex::Bool(t) => call("truth", t.span(), TokenStream::from(t.clone())),
            Ex::Hole(span) => call("hole", *span, TokenStream::new()),
            Ex::Place(p) => {
                let name = if cx == Cx::Arg { "arg" } else { "model_of" };
                call(name, place_span(p), self.borrow(p)?)
            }
            Ex::Path(t) => t.iter().cloned().collect(),
            Ex::Call(callee, paren, args) => {
                let mut out: TokenStream = callee.iter().cloned().collect();
                out.extend([regroup(paren, self.list(args, Cx::Arg, paren.span())?)]);
                out
            }
            Ex::Method(recv, callee, paren, args) => {
                let mut out = self.expr(recv, Cx::Value)?;
                copy(&mut out, callee);
                out.extend([regroup(paren, self.list(args, Cx::Arg, paren.span())?)]);
                out
            }
            Ex::Field(base, dot, name) => {
                let mut out = self.expr(base, Cx::Value)?;
                out.extend([dot.clone(), name.clone()]);
                out
            }
            Ex::Index(base, index, span) => {
                let mut args = self.expr(base, Cx::Value)?;
                args.extend([comma(*span)]);
                args.extend(self.expr(index, Cx::Arg)?);
                call("index", *span, args)
            }
            Ex::Unary(op, span, inner) => {
                if let ('-', Some(lit)) = (*op, literal(inner)) {
                    // A negative literal is an `Int`.
                    let mut l = sk("lit", lit.span());
                    l.extend(parse("::<::locus::Int>", lit.span()));
                    let mut g = Group::new(Delimiter::Parenthesis, TokenStream::from(lit.clone()));
                    g.set_span(lit.span());
                    l.extend([TT::Group(g)]);
                    return Ok(call("neg", *span, l));
                }
                let name = if *op == '!' { "not" } else { "neg" };
                call(name, *span, self.expr(inner, Cx::Value)?)
            }
            Ex::Binary(op, span, l, r) => {
                let name = match *op {
                    "<==>" => "iff",
                    "==>" => "implies",
                    "||" => "or",
                    "&&" => "and",
                    "===" => "same",
                    "!==" => "differ",
                    "==" => "eq",
                    "!=" => "ne",
                    "<" => "lt",
                    "<=" => "le",
                    ">" => "gt",
                    ">=" => "ge",
                    "+" => "add",
                    "-" => "sub",
                    "*" => "mul",
                    "/" => "div",
                    _ => "rem",
                };
                // An integer literal beside another operand has that
                // operand's type. Rust has no default for a literal of a
                // type that is not its own, and an operator with two
                // implementations for `Nat` would leave it ambiguous.
                let sibling = level(op) >= 5 && !matches!(*op, "===" | "!==");
                // Where both operands are literals nothing gives their
                // type, and Locus's rule makes them `Int`s.
                if let (true, Some(a), Some(b)) = (level(op) >= 5, literal(l), literal(r)) {
                    let mut args = sk("lit", a.span());
                    args.extend(parse("::<::locus::Int>", a.span()));
                    let mut g = Group::new(Delimiter::Parenthesis, TokenStream::from(a.clone()));
                    g.set_span(a.span());
                    args.extend([TT::Group(g), comma(*span)]);
                    let mut g = Group::new(Delimiter::Parenthesis, TokenStream::from(b.clone()));
                    g.set_span(b.span());
                    args.extend(sk("lit", b.span()));
                    args.extend(parse("::<::locus::Int>", b.span()));
                    args.extend([TT::Group(g)]);
                    return Ok(call(name, *span, args));
                }
                match (sibling, literal(l), literal(r)) {
                    (true, None, Some(lit)) | (true, Some(lit), None) => {
                        let left_is_lit = literal(l).is_some();
                        let other = if left_is_lit { r } else { l };
                        let (a, b) = if left_is_lit {
                            ("__locus_r", "__locus_l")
                        } else {
                            ("__locus_l", "__locus_r")
                        };
                        let mut inner = parse(&format!("let {a} ="), *span);
                        inner.extend(self.expr(other, Cx::Value)?);
                        inner.extend(parse(&format!("; let {b} ="), *span));
                        let mut args = parse(&format!("&{a},"), lit.span());
                        args.extend([lit.clone()]);
                        inner.extend(call("lit_like", lit.span(), args));
                        inner.extend(parse(";", *span));
                        inner.extend(call(name, *span, parse("__locus_l, __locus_r", *span)));
                        braces(*span, inner)
                    }
                    _ => {
                        let mut args = self.expr(l, Cx::Value)?;
                        args.extend([comma(*span)]);
                        args.extend(self.expr(r, Cx::Value)?);
                        call(name, *span, args)
                    }
                }
            }
            Ex::If(span, cond, then, els) => {
                let mut out = parse("if", *span);
                out.extend(call("cond", span_of(cond), self.expr(cond, Cx::Value)?));
                out.extend(braces(then.span, self.block_inner(then, cx)?));
                out.extend(parse("else", *span));
                match &**els {
                    Ex::Block(b) => out.extend(braces(b.span, self.block_inner(b, cx)?)),
                    other => out.extend(braces(*span, self.expr(other, cx)?)),
                }
                out
            }
            Ex::Match(span, scrutinee, body, arms) => {
                let mut out = parse("match", *span);
                out.extend(call(
                    "view",
                    span_of(scrutinee),
                    self.expr(scrutinee, Cx::Value)?,
                ));
                let mut inner = TokenStream::new();
                for arm in arms {
                    inner.extend(arm.pat.clone());
                    if let Some(g) = &arm.guard {
                        inner.extend(parse("if", *span));
                        inner.extend(call("cond", span_of(g), self.expr(g, Cx::Value)?));
                    }
                    inner.extend(parse("=>", *span));
                    let mut arm_body = TokenStream::new();
                    for name in &arm.proofs {
                        arm_body.extend(parse("#[allow(unused_variables)] let", name.span()));
                        arm_body.extend([name.clone()]);
                        arm_body.extend(parse("= ::locus::Erased;", name.span()));
                    }
                    arm_body.extend(self.expr(&arm.body, cx)?);
                    inner.extend(braces(span_of(&arm.body), arm_body));
                    inner.extend([comma(*span)]);
                }
                out.extend([regroup(body, inner)]);
                out
            }
            Ex::Struct(path, brace, fields) => {
                let mut out: TokenStream = path.iter().cloned().collect();
                let mut inner = TokenStream::new();
                for (name, value) in fields {
                    inner.extend([name.clone()]);
                    inner.extend(parse(":", name.span()));
                    inner.extend(self.expr(value, Cx::Arg)?);
                    inner.extend([comma(name.span())]);
                }
                out.extend([regroup(brace, inner)]);
                out
            }
            Ex::Tuple(g, items) => {
                TokenStream::from(regroup(g, self.list(items, Cx::Value, g.span())?))
            }
            Ex::Paren(g, inner) => TokenStream::from(regroup(g, self.expr(inner, cx)?)),
            Ex::Block(b) => braces(b.span, self.block_inner(b, cx)?),
            Ex::Lambda(span, params, count, body) => {
                let mut closure = parse("|", *span);
                copy(&mut closure, params);
                closure.extend(parse("|", *span));
                closure.extend(self.expr(body, Cx::Value)?);
                call(&format!("lambda{count}"), *span, closure)
            }
            Ex::Quant(kind, span, params, count, body) => {
                let mut closure = parse("|", *span);
                copy(&mut closure, params);
                closure.extend(parse("|", *span));
                closure.extend(self.expr(body, Cx::Value)?);
                call(&format!("{kind}{count}"), *span, closure)
            }
            Ex::Snap(span, p) => call("snap", *span, self.borrow(p)?),
            Ex::Prove(span, p) => {
                let mut closure = parse("||", *span);
                closure.extend(self.expr(p, Cx::Value)?);
                call("proved", *span, closure)
            }
        })
    }
}
