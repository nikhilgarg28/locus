//! Building data and taking it apart: tuples, struct literals, enum
//! variants, and the fields of a struct or a tuple.

use crate::ast::{self, ExprKind};
use crate::kernel::{Term, Type, VarId, telescope_entry};
use crate::source::Span;
use crate::typed::Expr;

use super::env::{Elab, EnumInfo, Env, Global};
use super::exprs::Value;

impl Env<'_> {
    pub(super) fn tuple(
        &mut self,
        items: &[ast::Expr],
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        let telescope = match expected {
            Some(Type::Tuple(fields)) if fields.len() == items.len() => expected.cloned(),
            _ => None,
        };
        let mut fields = Vec::new();
        let mut terms = Vec::new();
        let mut tys = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let value = match &telescope {
                Some(telescope) => {
                    let ty = telescope_entry(telescope, index, &terms)
                        .expect("the index is within the telescope");
                    self.check(item, &ty)?
                }
                None => self.infer(item)?,
            };
            terms.push(self.term(&value, item.span)?);
            tys.push(value.ty);
            fields.push(value.expr);
        }
        let ty = telescope.unwrap_or(Type::Tuple(tys));
        let _ = span;
        Ok(Value::new(
            Expr::Tuple {
                ty: ty.clone(),
                fields,
            },
            ty,
        ))
    }

    pub(super) fn struct_literal(
        &mut self,
        name: &ast::Name,
        fields: &[ast::ValueField],
        span: Span,
    ) -> Elab<Value> {
        let Some(Global::Struct(info)) = self.globals.get(&name.text).cloned() else {
            if self.failed.contains(&name.text) {
                return Err(());
            }
            return self.fail(
                "L0204",
                format!("unknown struct `{}`", name.text),
                name.span,
            );
        };
        let mut given: Vec<(String, &ast::Expr, Span)> = Vec::new();
        for field in fields {
            let field_name = match (&field.name, &field.value.kind) {
                (Some(name), _) => name.text.clone(),
                (None, ExprKind::Name(name)) => name.text.clone(),
                (None, _) => {
                    return self.fail("L0224", "a field needs a name: `name: value`", field.span);
                }
            };
            if given.iter().any(|(earlier, _, _)| *earlier == field_name) {
                return self.fail(
                    "L0224",
                    format!("field `{field_name}` is given twice"),
                    field.span,
                );
            }
            if !info
                .fields
                .iter()
                .any(|declared| declared.name == field_name)
            {
                let message = format!("`{}` has no field `{field_name}`", info.name);
                return self.fail("L0224", message, field.span);
            }
            given.push((field_name, &field.value, field.span));
        }
        let mut exprs = Vec::new();
        let mut tys: Vec<Type> = info.fields.iter().map(|field| field.ty.clone()).collect();
        for (index, declared) in info.fields.iter().enumerate() {
            let Some((_, value, _)) = given.iter().find(|(name, _, _)| *name == declared.name)
            else {
                let message = format!("missing field `{}` of `{}`", declared.name, info.name);
                return self.fail("L0224", message, span);
            };
            let value = self.check(value, &tys[index].clone())?;
            let term = self.term(&value, span)?;
            for later in tys[index + 1..].iter_mut() {
                *later = later.replace_var(declared.id, &term);
            }
            exprs.push((declared.name.clone(), value.expr));
        }
        Ok(Value::new(
            Expr::Struct {
                id: info.id,
                name: info.name.clone(),
                fields: exprs,
            },
            Type::Struct(info.id),
        ))
    }

    pub(super) fn member(
        &mut self,
        expr: &ast::Expr,
        value: &ast::Expr,
        name: &ast::Name,
    ) -> Elab<Value> {
        let target = self.infer(value)?;
        let Type::Struct(id) = &target.ty else {
            let shown = self.show_type(&target.ty);
            self.diagnostics.push(
                crate::diagnostic::Diagnostic::error(
                    "L0210",
                    format!("`{shown}` has no field `{}`", name.text),
                    name.span,
                )
                .note(
                    "a tuple is projected by position, as in `pair.0`, or taken apart with `let`",
                ),
            );
            return Err(());
        };
        let info = self.struct_by_id(*id).expect("a struct type was declared");
        let Some(index) = info.fields.iter().position(|field| field.name == name.text) else {
            let message = format!("`{}` has no field `{}`", info.name, name.text);
            return self.fail("L0210", message, name.span);
        };
        self.field(target, index, Some(name.text.clone()), expr.span)
    }

    pub(super) fn index(
        &mut self,
        expr: &ast::Expr,
        value: &ast::Expr,
        index: &str,
        index_span: &Span,
    ) -> Elab<Value> {
        let target = self.infer(value)?;
        let arity = match &target.ty {
            Type::Tuple(fields) => fields.len(),
            _ => 0,
        };
        match index.parse::<usize>() {
            Ok(position) if position < arity => self.field(target, position, None, expr.span),
            _ => {
                let shown = self.show_type(&target.ty);
                self.fail(
                    "L0210",
                    format!("`{shown}` has no field `{index}`"),
                    *index_span,
                )
            }
        }
    }

    fn field(
        &mut self,
        target: Value,
        index: usize,
        name: Option<String>,
        span: Span,
    ) -> Elab<Value> {
        let term = self.term(&target, span)?;
        // The kernel knows what the field's type says about the other fields.
        let ty = self.type_of(&Term::proj(term, index), span)?;
        Ok(Value::new(
            Expr::Field {
                target: Box::new(target.expr),
                index,
                name,
                ty: ty.clone(),
            },
            ty,
        ))
    }

    pub(super) fn enum_variant(
        &mut self,
        path: &ast::Path,
    ) -> Elab<Option<(std::rc::Rc<EnumInfo>, usize)>> {
        match self.globals.get(&path.prefix.text).cloned() {
            Some(Global::Enum(info)) => {
                match info
                    .variants
                    .iter()
                    .position(|(name, _)| *name == path.name.text)
                {
                    Some(index) => Ok(Some((info, index))),
                    None => {
                        let message =
                            format!("`{}` has no variant `{}`", info.name, path.name.text);
                        self.fail("L0212", message, path.name.span)
                    }
                }
            }
            Some(Global::Prop(_)) => Ok(None),
            Some(_) => self.fail(
                "L0212",
                format!("`{}` is not an enum or a proposition", path.prefix.text),
                path.prefix.span,
            ),
            None if self.failed.contains(&path.prefix.text) => Err(()),
            None => self.fail(
                "L0204",
                format!("unknown name `{}`", path.prefix.text),
                path.prefix.span,
            ),
        }
    }

    pub(super) fn variant(
        &mut self,
        path: &ast::Path,
        arguments: &[ast::Expr],
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        let Some((info, index)) = self.enum_variant(path)? else {
            let Some(Global::Prop(info)) = self.globals.get(&path.prefix.text).cloned() else {
                unreachable!("`enum_variant` saw a proposition")
            };
            return self.construct(&info, path, arguments, expected, span);
        };
        let payload = &info.variants[index].1;
        let ids: Vec<VarId> = payload.iter().map(|binder| binder.id).collect();
        let mut tys: Vec<Type> = payload.iter().map(|binder| binder.ty.clone()).collect();
        let what = format!("`{}::{}`", info.name, path.name.text);
        let payload = self.arguments(arguments, &ids, &mut tys, &what, span)?;
        Ok(Value::new(
            Expr::Variant {
                id: info.id,
                enum_name: info.name.clone(),
                index,
                variant_name: path.name.text.clone(),
                payload,
            },
            Type::Enum(info.id),
        ))
    }
}
