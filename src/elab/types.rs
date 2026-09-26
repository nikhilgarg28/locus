//! Surface types to kernel types.
//!
//! Logical classification belongs to the surface type. The kernel's Bool
//! represents both runtime bool and logical Bool; an erasure shape retains
//! their distinction through aggregates. Ghost is accepted only far enough
//! to emit a migration diagnostic; it is never a type constructor.

use crate::ast;
use crate::kernel::{Type, VarId};
use crate::typed::Binder;

use super::env::{Elab, Env, Global};

/// A type as written where a binding is declared: its kernel type, and
/// whether its kernel representation needs an explicit logical mode.
pub(super) struct Written {
    pub ty: Type,
    pub ghost: bool,
}

impl Env<'_> {
    /// A type in a binding position, including its logical representation.
    pub fn written(&mut self, ty: &ast::Type) -> Elab<Written> {
        match &ty.kind {
            ast::TypeKind::Group(inner) => self.written(inner),
            ast::TypeKind::Path { path, arguments }
                if path.single().is_some_and(|name| name.text == "Ghost") =>
            {
                let replacement = arguments.first().and_then(|inner| match &inner.kind {
                    ast::TypeKind::Named(name) if self.machine_type(&name.text).is_some() => {
                        Some("Int".to_string())
                    }
                    ast::TypeKind::Named(name) if name.text == "bool" => Some("Bool".to_string()),
                    _ => None,
                });
                self.migrate_logical_type(ty.span, replacement)
            }
            _ => {
                let kernel_type = self.ty(ty)?;
                let ghost = self.logical_spelling(ty) && !kernel_type.is_ghost();
                Ok(Written {
                    ty: kernel_type,
                    ghost,
                })
            }
        }
    }

    /// The annotation of a `let`: as `written`, and `Int` may stand there
    /// in any function, since a `let` of type `Int` never runs.
    pub fn let_annotation(&mut self, ty: &ast::Type) -> Elab<Written> {
        match &ty.kind {
            ast::TypeKind::Named(name) if name.text == "Int" => Ok(Written {
                ty: Type::Int,
                ghost: false,
            }),
            _ => self.written(ty),
        }
    }

    /// A type in any other position, where `Ghost<T>` may not stand.
    pub fn ty(&mut self, ty: &ast::Type) -> Elab<Type> {
        match &ty.kind {
            ast::TypeKind::Scoped { name, claims } => {
                let base = self.ty(&ast::Type { kind: ast::TypeKind::Named(name.clone()), span: ty.span })?;
                let was_total = std::mem::replace(&mut self.total, true);
                let indices = claims.iter().map(|claim| self.formula(claim)).collect::<Elab<Vec<_>>>();
                self.total = was_total;
                Ok(Type::Instance(Box::new(base), indices?.into()))
            }
            ast::TypeKind::Lifetime(_) => self.fail("L0201", "a lifetime is an argument of a nominal type, not a value type", ty.span),
            ast::TypeKind::Array { element, length } => { self.array_length(length)?; self.collection_type(element,ty.span) },
            ast::TypeKind::Slice(_) => self.fail("L0284", "slices are currently permitted only as reference parameters", ty.span),
            ast::TypeKind::Named(name) => match name.text.as_str() {
                "bool" => Ok(Type::Bool),
                "Bool" => {
                    self.require_preview(crate::preview::Feature::LogicalSplit, "Bool", name.span)?;
                    Ok(Type::Bool)
                },
                "Prop" => Ok(Type::Prop),
                machine if self.machine_type(machine).is_some() => {
                    Ok(Type::machine(self.machine_type(machine).unwrap()))
                }
                // The integers of the logic have no runtime form: they are
                // written where nothing runs, in a proposition, a function
                // of the logic, or a proof type.
                "Int" => Ok(Type::Int),

                wide @ ("u128" | "i128") => self.fail(
                    "L0290",
                    format!("the type `{wide}` is not in Locus yet"),
                    name.span,
                ),
                "Self" if self.owner.is_none() => self.fail(
                    "L0200",
                    "`Self` is the type of an `impl` block, and this is outside one",
                    name.span,
                ),
                // `Self` inside an `impl` block is its type.
                _ => match self
                    .types
                    .get(&self.type_text(name))
                    .or_else(|| self.values.get(&name.text))
                {
                    Some(Global::Struct(info)) => Ok(Type::Struct(info.id)),
                    Some(Global::Enum(info)) => Ok(Type::Enum(info.id)),
                    Some(Global::Prop(info)) => {
                        let message = format!(
                            "`{}` is a proposition, not a type; its proofs have type `@{}(...)`",
                            info.name, info.name
                        );
                        self.fail("L0200", message, name.span)
                    }
                    Some(Global::Fn(_)) => self.fail(
                        "L0200",
                        format!("`{}` is a function, not a type", name.text),
                        name.span,
                    ),
                    None => {
                        if self.failed.contains(&self.type_text(name)) {
                            return Err(());
                        }
                        self.fail(
                            "L0200",
                            format!("unknown type `{}`", name.text),
                            name.span,
                        )
                    }
                },
            },
            ast::TypeKind::Path { path, arguments } if path.single().is_some_and(|name|name.text=="Box") => {
                self.require_preview(crate::preview::Feature::HeapViews,"Box type",ty.span)?;
                let [inner]=arguments.as_slice() else{return self.fail("L0201","Box requires one payload type",ty.span)};
                Ok(Type::Boxed(Box::new(self.ty(inner)?)))
            },
            ast::TypeKind::Path { path, arguments } if path.single().is_some_and(|name| name.text == "Vec") => {
                let [element] = arguments.as_slice() else { return self.fail("L0284", "Vec requires one runtime element type", ty.span); };
                self.collection_type(element, ty.span)
            },
            ast::TypeKind::Path { path, arguments } if !arguments.is_empty() && arguments.iter().all(|a| matches!(a.kind,ast::TypeKind::Lifetime(_))) => {
                if let Some(name) = path.single() { self.ty(&ast::Type { kind: ast::TypeKind::Named(name.clone()), span:ty.span }) }
                else { self.fail("L0201","a lifetime application must name a type",ty.span) }
            },
            ast::TypeKind::Path { path, .. }
                if path.single().is_some_and(|name| name.text == "Ghost") =>
            { self.written(ty).map(|written| written.ty) }
            ast::TypeKind::Path { path, .. } if path.single().is_some() => self.fail(
                "L0290",
                "this type application is not available; enable its preview or declare the generic type",
                ty.span,
            ),
            ast::TypeKind::Path { .. } => self.fail(
                "L0290",
                "paths through modules are not in Locus yet; modules are a later project",
                ty.span,
            ),
            ast::TypeKind::Unit => Ok(Type::Tuple(Vec::new())),
            ast::TypeKind::Group(inner) => self.ty(inner),
            ast::TypeKind::Tuple(fields) => {
                let mark = self.mark();
                let binders = self.telescope(
                    fields
                        .iter()
                        .map(|field| (field.name.as_ref(), &field.ty, field.span)),
                    false,
                );
                self.close(mark);
                Ok(tuple_over(&binders?))
            }
            ast::TypeKind::Proof(proposition) => {
                let was_total = std::mem::replace(&mut self.total, true);
                let claim = self.formula(proposition);
                self.total = was_total;
                Ok(Type::proof(claim?))
            }
            // The type of a function of the logic: a value of it is applied
            // in a proposition, and nothing runs it.
            ast::TypeKind::LogicalFunction { parameters, result } => {
                self.require_preview(crate::preview::Feature::LogicalData, "logical callable type (LOC-225)", ty.span)?;
                if parameters.iter().any(|field| !self.logical_spelling(field.ty.observed())) || !self.logical_spelling(result) {
                    return self.fail("L0270", "a logical callable must take and return Logical types", ty.span);
                }
                let mark = self.mark();
                let value = self.logical("a logical callable type", |env| {
                    let binders = env.telescope(parameters.iter().map(|field| (field.name.as_ref(), field.ty.observed(), field.span)), true)?;
                    let result = env.ty(result)?;
                    Ok(Type::function_over(&pairs(&binders), &result))
                });
                self.close(mark);
                value
            }
            ast::TypeKind::Function { parameters, result } => {
                let mark = self.mark();
                let signature = (|| {
                    let binders = self.telescope(
                        parameters
                            .iter()
                            .map(|field| (field.name.as_ref(), &field.ty, field.span)),
                        false,
                    )?;
                    let result = self.ty(result)?;
                    Ok(Type::function_over(&pairs(&binders), &result))
                })();
                self.close(mark);
                signature
            }
            ast::TypeKind::Ref { mutable:true, .. } => self.fail("L0285", "stored mutable references are not supported; lend &mut for one call", ty.span),
            ast::TypeKind::Ref { inner, .. } => {
                self.require_preview(crate::preview::Feature::HeapViews,"stored shared reference",ty.span)?;
                if let ast::TypeKind::Slice(element) = &inner.kind { self.collection_type(element,inner.span) }
                else { self.ty(inner) }
            },
            ast::TypeKind::Never => self.fail(
                "L0290",
                "the never type `!` stands only as the result type of a function that never returns; anywhere else it is not in Locus, as it is not in stable Rust",
                ty.span,
            ),
        }
    }

    /// Elaborates fields in order, bringing each named one into scope for the
    /// fields after it. The caller decides when that scope ends. `ghosts`
    /// is whether a field may be declared `Ghost<T>`: the fields of a
    /// struct or a variant may, the fields of a tuple type may not.
    pub fn telescope<'t>(
        &mut self,
        fields: impl Iterator<Item = (Option<&'t ast::Name>, &'t ast::Type, crate::source::Span)>,
        ghosts: bool,
    ) -> Elab<Vec<Binder>> {
        let mut binders = Vec::new();
        for (name, ty, span) in fields {
            let written = if ghosts {
                self.written(ty)?
            } else {
                Written {
                    ty: self.ty(ty)?,
                    ghost: false,
                }
            };
            let binder = Binder {
                id: VarId::fresh(),
                name: name.map_or_else(|| "_".to_string(), |name| name.text.clone()),
                ty: written.ty,
                ghost: written.ghost,
            };
            self.session
                .register_binding_layout(binder.id, self.written_layout(ty));
            if let Some(name) = name
                && binders
                    .iter()
                    .any(|earlier: &Binder| earlier.name == name.text)
            {
                return self.fail(
                    "L0202",
                    format!("`{}` is declared twice", name.text),
                    name.span,
                );
            }
            let result = self
                .ctx
                .declare_with(binder.id, binder.ty.clone(), binder.ghost);
            self.kernel(result, span)?;
            if name.is_some() {
                self.bind(&binder.name, binder.id, &binder.ty, binder.ghost);
            }
            binders.push(binder);
        }
        Ok(binders)
    }
}

pub(super) fn pairs(binders: &[Binder]) -> Vec<(VarId, Type)> {
    binders
        .iter()
        .map(|binder| (binder.id, binder.ty.clone()))
        .collect()
}

pub(super) fn tuple_over(binders: &[Binder]) -> Type {
    Type::tuple_over(&pairs(binders))
}
