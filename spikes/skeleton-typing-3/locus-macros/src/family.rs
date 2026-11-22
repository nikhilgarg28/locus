//! Round 3: the family, by one implementation for each type.
//!
//! Where `locus!` sees a struct or an enum declared, it writes the type's
//! snapshot type, `Name_Snap`, and exactly one `unsafe impl ::locus::Model`:
//!
//! - with no model declared for the type in the same invocation, the model
//!   is the snapshot (`type Logic = Self::Snap`, and `model` is the
//!   identity);
//! - with `impl Model for Name { type Logic = ..; logic fn model(..) }` in
//!   the same invocation, that is the one implementation, and this module
//!   supplies `type Snap` and `valid` to it;
//! - with `#[derive(Model)]`, the model is the derived type.
//!
//! The snapshot type is generic over the snapshot types of the fields, one
//! parameter for each field, so its declaration names no type of the
//! program and needs no bound: `Stack_Snap<F0, F1> { items: F0, cap: F1 }`.
//! The implementation gives each parameter: `<Vec<u32> as Model>::Snap`.
//! No lifetime of the type reaches the snapshot, because rustc normalizes
//! `<&'a T as Model>::Snap` to `T`'s.
//!
//! Everything here is from the tokens of the declaration alone. GUESS marks
//! each place where that is not enough.
use crate::*;

#[derive(Clone)]
pub(crate) struct Field {
    /// A field's name, or its index in a tuple.
    pub name: String,
    pub vis: Vec<TT>,
    pub ty: Vec<TT>,
    /// Marked `#[opaque]`: its snapshot is `Opaque<Type>`.
    pub opaque: bool,
    /// Its type is a proposition: its snapshot is the proof.
    pub proof: bool,
    pub variant: usize,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Shape {
    Named,
    Tuple,
    Unit,
}

#[derive(Clone)]
pub(crate) struct Variant {
    pub name: String,
    pub shape: Shape,
}

#[derive(Clone)]
pub(crate) struct Adt {
    pub name: String,
    pub is_enum: bool,
    /// `<..>` with its brackets, or nothing.
    pub generics: Vec<TT>,
    /// The predicates of its `where` clause.
    pub wher: Vec<TT>,
    /// One for a struct.
    pub variants: Vec<Variant>,
    pub fields: Vec<Field>,
}

fn is_opaque_attr(toks: &[TT], i: usize) -> bool {
    punct(toks.get(i), '#')
        && group(toks.get(i + 1), Delimiter::Bracket)
            .is_some_and(|g| g.stream().to_string().trim() == "opaque")
}

/// Tokens without `#[opaque]`, which is Locus's and not an attribute rustc knows.
pub(crate) fn strip_opaque(toks: &[TT]) -> Vec<TT> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        if is_opaque_attr(toks, i) {
            i += 2;
            continue;
        }
        match &toks[i] {
            TT::Group(g) => out.push(regroup(
                g,
                strip_opaque(&collect(g.stream())).into_iter().collect(),
            )),
            t => out.push(t.clone()),
        }
        i += 1;
    }
    out
}

fn fields_of(inner: &[TT], named: bool, variant: usize, out: &mut Vec<Field>) {
    let mut index = 0;
    for (part, _) in split_top(inner, ',') {
        let mut a = 0;
        let mut opaque = false;
        while punct(part.get(a), '#') && group(part.get(a + 1), Delimiter::Bracket).is_some() {
            opaque |= is_opaque_attr(part, a);
            a += 2;
        }
        let v = skip_vis(part, a);
        let vis: Vec<TT> = lower_vis(part, a).into_iter().collect();
        let (name, ty) = if named {
            let Some(c) = (v..part.len()).find(|&c| single_colon(part, c)) else {
                continue;
            };
            (part[v].to_string(), part[c + 1..].to_vec())
        } else {
            (index.to_string(), part[v..].to_vec())
        };
        if ty.is_empty() {
            continue;
        }
        index += 1;
        let proof = punct(ty.first(), '@');
        out.push(Field {
            name,
            vis,
            ty,
            opaque,
            proof,
            variant,
        });
    }
}

/// A struct or an enum, from `struct Name ..` or `enum Name ..`.
pub(crate) fn parse_adt(toks: &[TT]) -> Option<Adt> {
    let is_enum = ident(toks.first(), "enum");
    if !is_enum && !ident(toks.first(), "struct") {
        return None;
    }
    let name = ident_name(toks.get(1))?;
    let (generics, after) = generics_at(toks, 2);
    let rest = &toks[after.min(toks.len())..];
    let mut wher = Vec::new();
    if let Some(w) = rest.iter().position(|t| ident(Some(t), "where")) {
        for t in &rest[w + 1..] {
            if punct(Some(t), ';') || group(Some(t), Delimiter::Brace).is_some() {
                break;
            }
            wher.push(t.clone());
        }
    }
    let mut adt = Adt {
        name: name.clone(),
        is_enum,
        generics: generics.to_vec(),
        wher,
        variants: Vec::new(),
        fields: Vec::new(),
    };
    if is_enum {
        let body = rest.iter().find_map(|t| group(Some(t), Delimiter::Brace))?;
        let inner = collect(body.stream());
        for (part, _) in split_top(&inner, ',') {
            let mut a = 0;
            while punct(part.get(a), '#') && group(part.get(a + 1), Delimiter::Bracket).is_some() {
                a += 2;
            }
            let Some(variant) = ident_name(part.get(a)) else {
                continue;
            };
            let k = adt.variants.len();
            let shape = match any_group(part.get(a + 1)) {
                Some(g) if g.delimiter() == Delimiter::Parenthesis => {
                    fields_of(&collect(g.stream()), false, k, &mut adt.fields);
                    Shape::Tuple
                }
                Some(g) if g.delimiter() == Delimiter::Brace => {
                    fields_of(&collect(g.stream()), true, k, &mut adt.fields);
                    Shape::Named
                }
                _ => Shape::Unit,
            };
            adt.variants.push(Variant {
                name: variant,
                shape,
            });
        }
    } else {
        let shape = match rest.first() {
            Some(TT::Group(g)) if g.delimiter() == Delimiter::Parenthesis => {
                fields_of(&collect(g.stream()), false, 0, &mut adt.fields);
                Shape::Tuple
            }
            _ => match rest.iter().find_map(|t| group(Some(t), Delimiter::Brace)) {
                Some(g) => {
                    fields_of(&collect(g.stream()), true, 0, &mut adt.fields);
                    Shape::Named
                }
                None => Shape::Unit,
            },
        };
        adt.variants.push(Variant { name, shape });
    }
    Some(adt)
}

fn text(toks: &[TT]) -> String {
    toks.iter().cloned().collect::<TokenStream>().to_string()
}

/// Whether an identifier of `names` stands in the tokens.
fn mentions(toks: &[TT], names: &[String]) -> bool {
    toks.iter().any(|t| match t {
        TT::Ident(id) => names.contains(&id.to_string()),
        TT::Group(g) => mentions(&collect(g.stream()), names),
        _ => false,
    })
}

/// Every lifetime written `'static`: for a type that must name no lifetime
/// of the declaration. Lifetimes that the tokens do not show stay.
fn statics(ts: &[TT]) -> Vec<TT> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < ts.len() {
        match &ts[i] {
            TT::Punct(p) if p.as_char() == '\'' && ident_name(ts.get(i + 1)).is_some() => {
                out.extend(parse("'static", p.span()));
                i += 2;
            }
            TT::Group(g) => {
                out.push(regroup(g, statics(&collect(g.stream())).into_iter().collect()));
                i += 1;
            }
            t => {
                out.push(t.clone());
                i += 1;
            }
        }
    }
    out
}

/// The declaration's parameters replaced by an implementation's arguments:
/// `struct Shelf<T>` and `impl<U: Model> Model for Shelf<U>`.
fn substituted(ts: &[TT], from: &[String], to: &[Vec<TT>]) -> Vec<TT> {
    if from.is_empty() {
        return ts.to_vec();
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < ts.len() {
        match &ts[i] {
            TT::Punct(p) if p.as_char() == '\'' && ident_name(ts.get(i + 1)).is_some() => {
                let key = format!("'{}", ts[i + 1]);
                match from.iter().position(|n| *n == key) {
                    Some(k) if k < to.len() => out.extend(to[k].iter().cloned()),
                    _ => out.extend(ts[i..i + 2].iter().cloned()),
                }
                i += 2;
            }
            TT::Ident(id) => {
                let after_path = i > 0 && punct(ts.get(i - 1), ':');
                match from.iter().position(|n| *n == id.to_string()) {
                    Some(k) if k < to.len() && !after_path => out.extend(to[k].iter().cloned()),
                    _ => out.push(ts[i].clone()),
                }
                i += 1;
            }
            TT::Group(g) => {
                out.push(regroup(
                    g,
                    substituted(&collect(g.stream()), from, to).into_iter().collect(),
                ));
                i += 1;
            }
            t => {
                out.push(t.clone());
                i += 1;
            }
        }
    }
    out
}

impl Adt {
    /// The names of the type parameters.
    pub fn type_params(&self) -> Vec<String> {
        generic_parts(&self.generics)
            .into_iter()
            .filter(|p| !punct(p.first(), '\'') && !ident(p.first(), "const"))
            .filter_map(|p| ident_name(p.first()))
            .collect()
    }

    /// A field whose type names the type itself. Its snapshot cannot be a
    /// parameter of the snapshot type, which would then contain itself.
    /// GUESS: by the type's name, or `Self`; a type that contains itself
    /// through another is not seen.
    fn recursive(&self, f: &Field) -> bool {
        mentions(&f.ty, &[self.name.clone(), "Self".to_string()])
    }

    /// The fields whose snapshots are parameters of the snapshot type.
    fn parameters(&self) -> Vec<usize> {
        (0..self.fields.len())
            .filter(|&k| !self.recursive(&self.fields[k]))
            .collect()
    }

    /// The number of parameters of the snapshot type.
    pub fn arity(&self) -> usize {
        self.parameters().len()
    }

    /// Whether lowering can write the family for this type. Not for a
    /// generic type that contains itself: the field's snapshot would have to
    /// be written in terms of the type's own parameters.
    pub fn supported(&self) -> Result<(), &'static str> {
        if self.fields.iter().any(|f| self.recursive(f)) && !self.type_params().is_empty() {
            return Err("a generic type that contains itself");
        }
        Ok(())
    }

    /// A field's type as rustc reads it, with `Self` written out.
    fn field_type(&self, f: &Field) -> Vec<TT> {
        let lowered: Vec<TT> = lower_type(&f.ty).into_iter().collect();
        let own: Vec<TT> = parse(&self.name, Span::call_site()).into_iter().collect();
        written_self(&lowered, &own)
    }

    /// The type of a recursive field in the snapshot type's declaration:
    /// the projection itself, with no lifetime of the declaration. `own` is
    /// how the type is named there.
    fn direct(&self, f: &Field, own: &str) -> String {
        let ty = statics(&self.field_type(f));
        let ty = if own == self.name {
            ty
        } else {
            own_name_qualified(ty.into_iter().collect(), &self.name)
                .into_iter()
                .collect()
        };
        format!("<{} as ::locus::Model>::Snap", text(&ty))
    }

    /// The declaration of the snapshot type, and what every logical type has.
    pub fn snapshot(&self, vis: &str) -> String {
        let name = &self.name;
        let snap = format!("{name}_Snap");
        let params = self.parameters();
        let fs: Vec<String> = (0..params.len()).map(|k| format!("F{k}")).collect();
        let wrap = |v: &[String]| {
            if v.is_empty() {
                String::new()
            } else {
                format!("<{}>", v.join(", "))
            }
        };
        let plain = wrap(&fs);
        let bounded = wrap(
            &fs.iter()
                .map(|f| format!("{f}: ::locus::Logical"))
                .collect::<Vec<_>>(),
        );
        // The type of each field, in the snapshot type's declaration.
        let own = if self.is_enum { format!("self::{name}") } else { name.clone() };
        let ty_of = |k: usize| match params.iter().position(|p| *p == k) {
            Some(p) => format!("F{p}"),
            None => self.direct(&self.fields[k], &own),
        };
        const MARKER: &str = "::locus::Logical::__locus_marker()";
        let identity = |copy: &str, marker: &str| {
            format!(
                "impl{copy} ::core::clone::Clone for {snap}{plain} {{ fn clone(&self) -> Self {{ *self }} }}
                 impl{copy} ::core::marker::Copy for {snap}{plain} {{}}
                 impl{plain} ::core::fmt::Debug for {snap}{plain} {{
                     fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {{
                         f.write_str(\"Snap<{name}>\")
                     }}
                 }}
                 unsafe impl{bounded} ::locus::Model for {snap}{plain} {{
                     type Snap = Self;
                     type Logic = Self;
                     fn __logic_model(s: Self) -> Self {{ s }}
                     fn __logic_valid(_: Self) -> ::locus::Prop {{ ::locus::True }}
                 }}
                 impl{bounded} ::locus::Logical for {snap}{plain} {{
                     fn __locus_marker() -> Self {{ {marker} }}
                 }}"
            )
        };
        let lints = "#[allow(non_camel_case_types, dead_code, private_interfaces)]";
        if !self.is_enum {
            let (decl, marker) = match self.variants[0].shape {
                Shape::Named => {
                    let (mut fields, mut init) = (String::new(), String::new());
                    for (k, f) in self.fields.iter().enumerate() {
                        fields.push_str(&format!("{} {}: {}, ", text(&f.vis), f.name, ty_of(k)));
                        init.push_str(&format!("{}: {MARKER}, ", f.name));
                    }
                    (format!("{{ {fields} }}"), format!("{snap} {{ {init} }}"))
                }
                Shape::Tuple => {
                    let fields: Vec<String> = self
                        .fields
                        .iter()
                        .enumerate()
                        .map(|(k, f)| format!("{} {}", text(&f.vis), ty_of(k)))
                        .collect();
                    (
                        format!("({});", fields.join(", ")),
                        format!("{snap}({})", vec![MARKER; fields.len()].join(", ")),
                    )
                }
                Shape::Unit => (";".to_string(), snap.clone()),
            };
            return format!(
                "#[doc(hidden)] {lints} {vis} struct {snap}{plain} {decl}
                 {}
                 impl{plain} ::locus::logic::Matchable for {snap}{plain} {{ type Variants = Self; }}",
                identity(&bounded, &marker)
            );
        }
        // An enum's snapshot is a zero-sized struct with no payload, as a
        // logical enum is, and its variants are those of an enum that only
        // its `Matchable` names.
        let list: String = fs.iter().map(|f| format!("{f}, ")).collect();
        let (field, value) = if fs.is_empty() {
            (String::new(), String::new())
        } else {
            (
                format!("__locus: ::core::marker::PhantomData<fn() -> ({list})>"),
                "__locus: ::core::marker::PhantomData".to_string(),
            )
        };
        let mut variants = String::new();
        for (v, variant) in self.variants.iter().enumerate() {
            let fields: Vec<(usize, &Field)> = self
                .fields
                .iter()
                .enumerate()
                .filter(|(_, f)| f.variant == v)
                .collect();
            match variant.shape {
                Shape::Unit => variants.push_str(&format!("{}, ", variant.name)),
                Shape::Tuple => {
                    let tys: Vec<String> = fields.iter().map(|(k, _)| ty_of(*k)).collect();
                    variants.push_str(&format!("{}({}), ", variant.name, tys.join(", ")));
                }
                Shape::Named => {
                    let tys: Vec<String> = fields
                        .iter()
                        .map(|(k, f)| format!("{}: {}", f.name, ty_of(*k)))
                        .collect();
                    variants.push_str(&format!("{} {{ {} }}, ", variant.name, tys.join(", ")));
                }
            }
        }
        let phantom = if fs.is_empty() {
            String::new()
        } else {
            format!(
                "#[doc(hidden)] __locus(::core::marker::PhantomData<fn() -> ({list})>, ::core::convert::Infallible),"
            )
        };
        format!(
            "#[doc(hidden)] {lints} {vis} struct {snap}{plain} {{ {field} }}
             {}
             const _: () = {{
                 {lints}
                 pub enum {name}{plain} {{ {variants} {phantom} }}
                 impl{plain} ::locus::logic::Matchable for {snap}{plain} {{ type Variants = {name}{plain}; }}
             }};",
            identity(&plain, &format!("{snap} {{ {value} }}"))
        )
    }

    /// `(type Snap = ..; and fn __logic_valid, the where clauses they need)`
    /// for an implementation whose arguments are `to`, in place of the
    /// declaration's parameters `from`.
    fn members(&self, deferred: bool, from: &[String], to: &[Vec<TT>]) -> (String, Vec<String>) {
        let params = self.parameters();
        let types = self.type_params();
        let mut wheres = Vec::new();
        let mut args = Vec::new();
        // For each field: how its `valid` is called on a value of its
        // snapshot, or nothing.
        let mut valid: Vec<Option<String>> = Vec::new();
        for (k, f) in self.fields.iter().enumerate() {
            let generic = mentions(&f.ty, &types);
            let ty = substituted(&self.field_type(f), from, to);
            let (arg, check) = if f.proof {
                ("::locus::Proof".to_string(), None)
            } else if f.opaque {
                // The lifetimes that the tokens show are `'static`. A
                // parameter's own lifetimes cannot be seen: the snapshot
                // exists where `Opaque<..>` is a logical type.
                let opaque = format!("::locus::Opaque<{}>", text(&statics(&ty)));
                if generic {
                    wheres.push(format!("{opaque}: ::locus::Logical"));
                }
                (opaque, None)
            } else if deferred && !generic {
                // The conditional form: capture-family `c11`.
                let fixed = text(&statics(&ty));
                wheres.push(format!(
                    "for<'__locus_any> {}: ::locus::Deferred<'__locus_any>",
                    text(&ty)
                ));
                (
                    format!("<{fixed} as ::locus::Deferred<'static>>::Snap"),
                    Some(format!("<{fixed} as ::locus::Deferred<'static>>::__locus_valid")),
                )
            } else {
                if deferred {
                    wheres.push(format!("{}: ::locus::Model", text(&ty)));
                }
                (
                    format!("<{} as ::locus::Model>::Snap", text(&ty)),
                    Some(format!("<{} as ::locus::Model>::__logic_valid", text(&ty))),
                )
            };
            if params.contains(&k) {
                args.push(arg);
            }
            valid.push(check);
        }
        let snap = format!(
            "{}_Snap{}",
            self.name,
            if args.is_empty() {
                String::new()
            } else {
                format!("<{}>", args.join(", "))
            }
        );
        let and = |parts: Vec<String>| {
            parts
                .into_iter()
                .reduce(|a, b| format!("::locus::logic::and({a}, {b})"))
                .unwrap_or_else(|| "::locus::True".to_string())
        };
        let body = if !self.is_enum {
            and(self
                .fields
                .iter()
                .zip(&valid)
                .filter_map(|(f, v)| v.as_ref().map(|v| format!("{v}(__locus_s.{})", f.name)))
                .collect())
        } else {
            let mut arms = String::new();
            for (v, variant) in self.variants.iter().enumerate() {
                let fields: Vec<usize> = (0..self.fields.len())
                    .filter(|&k| self.fields[k].variant == v)
                    .collect();
                let checks = and(fields
                    .iter()
                    .filter_map(|&k| valid[k].as_ref().map(|v| format!("{v}(__locus_f{k})")))
                    .collect());
                let pattern = match variant.shape {
                    Shape::Unit => String::new(),
                    Shape::Tuple => format!(
                        "({})",
                        fields
                            .iter()
                            .map(|k| format!("__locus_f{k}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    Shape::Named => format!(
                        "{{ {} }}",
                        fields
                            .iter()
                            .map(|&k| format!("{}: __locus_f{k}", self.fields[k].name))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                };
                arms.push_str(&format!(
                    "::locus::logic::Variants::<{snap}>::{}{pattern} => {checks},\n",
                    variant.name
                ));
            }
            format!(
                "match ::locus::logic::matched(__locus_s) {{ {arms} #[allow(unreachable_patterns)] _ => ::locus::True }}"
            )
        };
        (
            format!(
                "type Snap = {snap};
                 #[allow(unused_variables)]
                 fn __logic_valid(__locus_s: Self::Snap) -> ::locus::Prop {{
                     ::locus::logic::logic_fn_prop(|| {body})
                 }}"
            ),
            wheres,
        )
    }

    /// The one implementation, where the program declares no model: the
    /// model is the snapshot. It is conditional on the type's parameters:
    /// `T: Model` for a parameter that a field's type names, and
    /// `F::Out: Model` for a field whose type is an associated type of one.
    /// GUESS: a parameter followed by `::`, or inside `<T as Trait>::Out`,
    /// is read as the start of an associated type.
    pub fn default_impl(&self, deferred: bool, logic: Option<(&str, &str)>) -> String {
        let name = &self.name;
        let (members, mut wheres) = self.members(deferred, &[], &[]);
        let types = self.type_params();
        let mut bounded: Vec<String> = Vec::new();
        if !deferred || logic.is_some() {
            for f in self.fields.iter().filter(|f| !f.opaque && !f.proof) {
                projections(&self.field_type(f), &types, &mut bounded, &mut wheres);
            }
        }
        if logic.is_some() {
            // A derived model is generic over every parameter's model.
            bounded = types.clone();
        }
        let mut params = Vec::new();
        for part in generic_parts(&self.generics) {
            let p = without_default(part);
            match ident_name(p.first()) {
                Some(t) if bounded.contains(&t) => params.push(if p.len() > 1 {
                    format!("{} + ::locus::Model", text(p))
                } else {
                    format!("{t}: ::locus::Model")
                }),
                _ => params.push(text(p)),
            }
        }
        let generics = if params.is_empty() {
            String::new()
        } else {
            format!("<{}>", params.join(", "))
        };
        if !self.wher.is_empty() {
            wheres.insert(0, text(&self.wher).trim_end_matches(',').to_string());
        }
        let wheres = if wheres.is_empty() {
            String::new()
        } else {
            format!("where {}", wheres.join(", "))
        };
        let (logic, model) = match logic {
            Some((ty, body)) => (ty.to_string(), body.to_string()),
            None => ("Self::Snap".to_string(), "__locus_s".to_string()),
        };
        format!(
            "unsafe impl{generics} ::locus::Model for {name}{} {wheres} {{
                 {members}
                 type Logic = {logic};
                 #[allow(unused_variables)]
                 fn __logic_model(__locus_s: Self::Snap) -> Self::Logic {{ {model} }}
             }}",
            generic_args(&self.generics)
        )
    }

    /// What lowering supplies to a model that the program declares:
    /// `type Snap`, `valid`, and, in the conditional form, `where` clauses.
    /// No bound is added: `impl<T> Model for Shelf<T>` without `T: Model` is
    /// rustc's error.
    pub fn declared_members(&self, deferred: bool, self_args: &[Vec<TT>]) -> (String, Vec<String>) {
        let from = generic_names(&self.generics);
        self.members(deferred, &from, self_args)
    }
}

/// The parameters that a field's type names bare, and the associated types
/// of parameters that it names.
fn projections(ty: &[TT], types: &[String], bare: &mut Vec<String>, wheres: &mut Vec<String>) {
    let mut i = 0;
    while i < ty.len() {
        match &ty[i] {
            // `<T as Trait>::Out`
            TT::Punct(p)
                if p.as_char() == '<'
                    && ident_name(ty.get(i + 1)).is_some_and(|n| types.contains(&n))
                    && ident(ty.get(i + 2), "as") =>
            {
                let (angle, next) = generics_at(ty, i);
                if punct(ty.get(next), ':') && punct(ty.get(next + 1), ':') && ident_name(ty.get(next + 2)).is_some() {
                    let whole = format!("{}::{}", text(angle), ty[next + 2]);
                    let clause = format!("{whole}: ::locus::Model");
                    if !wheres.contains(&clause) {
                        wheres.push(clause);
                    }
                    i = next + 3;
                } else {
                    i += 1;
                }
            }
            TT::Ident(id) if types.contains(&id.to_string()) => {
                let after_path = i > 0 && punct(ty.get(i - 1), ':');
                if punct(ty.get(i + 1), ':') && punct(ty.get(i + 2), ':') && ident_name(ty.get(i + 3)).is_some() {
                    let clause = format!("{id}::{}: ::locus::Model", ty[i + 3]);
                    if !wheres.contains(&clause) {
                        wheres.push(clause);
                    }
                    i += 4;
                } else {
                    if !after_path && !bare.contains(&id.to_string()) {
                        bare.push(id.to_string());
                    }
                    i += 1;
                }
            }
            TT::Group(g) => {
                projections(&collect(g.stream()), types, bare, wheres);
                i += 1;
            }
            _ => i += 1,
        }
    }
}

/// Tokens that carry the mark of this expansion, at `span`'s place in the
/// source: the driver tells an implementation that lowering wrote from one
/// that a program wrote by this mark, and rustc still reports an error in
/// it at the declaration.
pub(crate) fn marked(s: &str, span: Span) -> TokenStream {
    respan(s.parse().expect("toy: bad snippet"), Span::call_site().located_at(span))
}
