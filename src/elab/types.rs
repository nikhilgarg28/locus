//! Surface types to kernel types.

use crate::ast;
use crate::kernel::{Type, VarId};
use crate::typed::Binder;

use super::env::{Elab, Env, Global};

impl Env<'_> {
    pub fn ty(&mut self, ty: &ast::Type) -> Elab<Type> {
        match &ty.kind {
            ast::TypeKind::Named(name) => match name.text.as_str() {
                "bool" => Ok(Type::Bool),
                "u8" => Ok(Type::U8),
                "Prop" => Ok(Type::Prop),
                "Nat" | "Int" => {
                    self.diagnostics.push(
                        crate::diagnostic::Diagnostic::error(
                            "L0201",
                            format!("`{}` is not part of the core language", name.text),
                            name.span,
                        )
                        .note("the core has `bool` and `u8`; mathematical integers arrive in a later milestone"),
                    );
                    Err(())
                }
                other => match self.globals.get(other) {
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
            ast::TypeKind::Path { path, .. } if path.single().is_some() => self.fail(
                "L0290",
                "type arguments are not in Locus yet; E8 adds `Ghost<T>`",
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
            ast::TypeKind::Function {
                mode: ast::FunctionMode::Math,
                parameters,
                result,
            } => {
                let mark = self.mark();
                let signature = (|| {
                    let binders = self.telescope(
                        parameters
                            .iter()
                            .map(|field| (field.name.as_ref(), &field.ty, field.span)),
                    )?;
                    let result = self.ty(result)?;
                    Ok(Type::function_over(&pairs(&binders), &result))
                })();
                self.close(mark);
                signature
            }
            ast::TypeKind::Function { .. } => self.fail(
                "L0290",
                "values of `fn` type are not supported yet; a `math fn` type is",
                ty.span,
            ),
        }
    }

    /// Elaborates fields in order, bringing each named one into scope for the
    /// fields after it. The caller decides when that scope ends.
    pub fn telescope<'t>(
        &mut self,
        fields: impl Iterator<Item = (Option<&'t ast::Name>, &'t ast::Type, crate::source::Span)>,
    ) -> Elab<Vec<Binder>> {
        let mut binders = Vec::new();
        for (name, ty, span) in fields {
            let ty = self.ty(ty)?;
            let binder = Binder {
                id: VarId::fresh(),
                name: name.map_or_else(|| "_".to_string(), |name| name.text.clone()),
                ty,
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
            let result = self.ctx.declare_with(binder.id, binder.ty.clone(), false);
            self.kernel(result, span)?;
            if name.is_some() {
                self.bind(&binder.name, binder.id, &binder.ty);
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
