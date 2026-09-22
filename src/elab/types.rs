//! Surface types to kernel types.
//!
//! `Ghost<T>` is the one type constructor of the core: a value of it is a
//! logical value of type `T`, with no runtime form. The kernel has no
//! runtime/ghost distinction beyond its modes, so `Ghost<T>` elaborates to
//! the kernel type `T`, and what is `Ghost` is the binding: `written` says
//! so for the type of a parameter, a field, or a `let`, which is where
//! `Ghost<T>` stands, and the binder carries it (`typed::Binder::ghost`).
//! Such a binding is named only where nothing runs (`exprs.rs`), and
//! erasure makes it a marker, as evidence is.

use crate::ast;
use crate::diagnostic::Diagnostic;
use crate::kernel::{MachineInt, Type, VarId};
use crate::typed::Binder;

use super::env::{Elab, Env, Global};

/// A type as written where a binding is declared: its kernel type, and
/// whether the binding is `Ghost<T>`, for `ty` the kernel type `T`.
pub(super) struct Written {
    pub ty: Type,
    pub ghost: bool,
}

/// Whether a value of the type is logical data with no runtime form: a
/// proposition or an integer of the logic. Evidence is logic-only too, but
/// a call that returns it is the ordinary way of establishing a fact, and
/// is not elaborated where nothing runs.
pub(super) fn logical_data(ty: &Type) -> bool {
    matches!(ty, Type::Prop | Type::Int | Type::Nat)
}

impl Env<'_> {
    /// A type in a position that declares a binding, where `Ghost<T>` may
    /// stand: a parameter, a field, or a `let`.
    pub fn written(&mut self, ty: &ast::Type) -> Elab<Written> {
        match &ty.kind {
            ast::TypeKind::Group(inner) => self.written(inner),
            ast::TypeKind::Path { path, arguments }
                if path.single().is_some_and(|name| name.text == "Ghost") =>
            {
                if arguments.len() != 1 {
                    return self.fail(
                        "L0200",
                        "`Ghost` takes one type argument, `Ghost<T>`",
                        ty.span,
                    );
                }
                // `T` is the type of a logical value, so `Int` is fine in
                // it, and `Ghost<Ghost<T>>` is `Ghost<T>`.
                let was_total = std::mem::replace(&mut self.total, true);
                let inner = self.written(&arguments[0]);
                self.total = was_total;
                Ok(Written {
                    ty: inner?.ty,
                    ghost: true,
                })
            }
            _ => Ok(Written {
                ty: self.ty(ty)?,
                ghost: false,
            }),
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
            ast::TypeKind::Named(name) => match name.text.as_str() {
                "bool" => Ok(Type::Bool),
                "Prop" => Ok(Type::Prop),
                machine if MachineInt::from_name(machine).is_some() => {
                    Ok(Type::machine(MachineInt::from_name(machine).unwrap()))
                }
                // The integers of the logic have no runtime form: they are
                // written where nothing runs, in a proposition, a function
                // of the logic, or a proof type.
                "Int" if self.total => Ok(Type::Int),
                "Int" => {
                    self.diagnostics.push(
                        crate::diagnostic::Diagnostic::error(
                            "L0201",
                            "`Int` has no runtime form",
                            name.span,
                        )
                        .note("`Int` is the integers of the logic: it is written in a proposition, in a function that promises `terminates`, `no_panic`, and `no_io`, and in a proof type; at runtime a value has a machine integer type, `u8` to `i64`, and `x as Int` speaks of it in a claim"),
                    );
                    Err(())
                }
                "Nat" => {
                    self.diagnostics.push(
                        crate::diagnostic::Diagnostic::error(
                            "L0201",
                            "`Nat` is not part of the core language",
                            name.span,
                        )
                        .note("`Nat` is internal to the kernel; the integers of the logic are `Int`, and the machine integers are `u8` to `i64`"),
                    );
                    Err(())
                }
                wide @ ("u128" | "usize" | "i128" | "isize") => self.fail(
                    "L0290",
                    format!("the type `{wide}` is not in Locus yet"),
                    name.span,
                ),
                other => match self.types.get(other).or_else(|| self.values.get(other)) {
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
                        format!("`{other}` is a function, not a type"),
                        name.span,
                    ),
                    None => {
                        if self.failed.contains(other) {
                            return Err(());
                        }
                        self.fail("L0200", format!("unknown type `{other}`"), name.span)
                    }
                },
            },
            ast::TypeKind::Path { path, .. }
                if path.single().is_some_and(|name| name.text == "Ghost") =>
            {
                self.diagnostics.push(
                    Diagnostic::error(
                        "L0290",
                        "`Ghost<T>` cannot stand here; as a result type, or inside another type, it is not in Locus yet",
                        ty.span,
                    )
                    .note("`Ghost<T>` is the type of a `let`, a parameter, or a field: a logical value of type `T` that the program never computes"),
                );
                Err(())
            }
            ast::TypeKind::Path { path, .. } if path.single().is_some() => self.fail(
                "L0290",
                "type arguments are written only on `Ghost<T>` in the core; `Option<T>`, `Vec<T>`, and the rest are not in Locus yet",
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
            ast::TypeKind::Ref { .. } => self.fail(
                "L0290",
                "references (`&T`, `&mut T`) are not in Locus yet; O3 adds them",
                ty.span,
            ),
            // The result type of a function is read before `ty` is asked
            // (`items.rs`); anywhere else `!` is not stable Rust either.
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
