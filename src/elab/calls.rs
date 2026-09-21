//! Calls: of a function, a proposition, a method on `u8`, and the checking
//! of arguments against a telescope of parameter types.

use crate::ast::{self, ExprKind};
use crate::kernel::{Prim, Term, Type, VarId};
use crate::source::Span;
use crate::typed::{Expr, FnRef};

use super::env::{Elab, Env, FnInfo, Global, PropInfo};
use super::exprs::Value;

impl Env<'_> {
    /// Checks arguments against a telescope of parameter types, written over
    /// the identities `ids`: each argument's term replaces its parameter in
    /// the types that follow. On return `tys` no longer mentions `ids`.
    pub fn arguments(
        &mut self,
        arguments: &[ast::Expr],
        ids: &[VarId],
        tys: &mut [Type],
        what: &str,
        span: Span,
    ) -> Elab<Vec<Expr>> {
        if arguments.len() != ids.len() {
            let message = format!(
                "{what} takes {} value{}, and {} {} given",
                ids.len(),
                if ids.len() == 1 { "" } else { "s" },
                arguments.len(),
                if arguments.len() == 1 { "was" } else { "were" },
            );
            return self.fail("L0208", message, span);
        }
        let mut exprs = Vec::new();
        for (index, argument) in arguments.iter().enumerate() {
            let value = self.check(argument, &tys[index].clone())?;
            let term = self.term(&value, argument.span)?;
            for later in tys[index + 1..].iter_mut() {
                *later = later.replace_var(ids[index], &term);
            }
            // The last entry may be a result type.
            exprs.push(value.expr);
        }
        Ok(exprs)
    }

    pub(super) fn call(
        &mut self,
        callee: &ast::Expr,
        arguments: &[ast::Expr],
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        match &callee.kind {
            ExprKind::Path(path) => self.variant(path, arguments, expected, span),
            ExprKind::Member { value, name } => self.method(value, name, arguments, span),
            ExprKind::Name(name) if self.lookup(&name.text).is_none() => {
                match self.globals.get(&name.text).cloned() {
                    Some(Global::Fn(info)) => self.call_fn(&info, arguments, span),
                    Some(Global::Prop(info)) => {
                        let term = self.prop_application(&info, arguments, span)?;
                        Ok(Value::new(Expr::Prop(term), Type::Prop))
                    }
                    Some(_) => self.fail(
                        "L0207",
                        format!("`{}` cannot be called", name.text),
                        name.span,
                    ),
                    None if self.failed.contains(&name.text) => Err(()),
                    None if matches!(name.text.as_str(), "rewrite" | "unfold" | "fold") => {
                        self.proof_form(&name.text, arguments, expected, span)
                    }
                    None => self.fail(
                        "L0204",
                        format!("unknown function `{}`", name.text),
                        name.span,
                    ),
                }
            }
            _ => {
                let callee = self.infer(callee)?;
                self.apply(callee, arguments, span)
            }
        }
    }

    pub(super) fn call_fn(
        &mut self,
        info: &FnInfo,
        arguments: &[ast::Expr],
        span: Span,
    ) -> Elab<Value> {
        let ids: Vec<VarId> = info.params.iter().map(|param| param.id).collect();
        let mut tys: Vec<Type> = info.params.iter().map(|param| param.ty.clone()).collect();
        tys.push(info.result.clone());
        let what = format!("`{}`", info.name);
        // The result type rides along so that it is instantiated too.
        let count = ids.len();
        if arguments.len() != count {
            let mut only_params = tys[..count].to_vec();
            return self
                .arguments(arguments, &ids, &mut only_params, &what, span)
                .map(|_| unreachable!("the counts differ"));
        }
        let mut exprs = Vec::new();
        for (index, argument) in arguments.iter().enumerate() {
            let value = self.check(argument, &tys[index].clone())?;
            let term = self.term(&value, argument.span)?;
            for later in tys[index + 1..].iter_mut() {
                *later = later.replace_var(ids[index], &term);
            }
            exprs.push(value.expr);
        }
        let ty = tys.pop().expect("the result type was pushed");
        match info.reference {
            FnRef::Math(id) => Ok(Value::new(
                Expr::CallMath {
                    id,
                    name: info.name.clone(),
                    arguments: exprs,
                    ty: ty.clone(),
                },
                ty,
            )),
            FnRef::Exec(id) => {
                if self.total {
                    self.diagnostics.push(
                        crate::diagnostic::Diagnostic::error(
                            "L0209",
                            format!("`{}` is an ordinary `fn` and cannot be called here", info.name),
                            span,
                        )
                        .note("an ordinary `fn` may fail to return, so a `math fn` and a proposition can only call a `math fn`"),
                    );
                    return Err(());
                }
                let result = VarId::fresh();
                self.declare_result(result, &ty, span)?;
                Ok(Value::new(
                    Expr::CallFn {
                        id,
                        name: info.name.clone(),
                        arguments: exprs,
                        result,
                        ty: ty.clone(),
                    },
                    ty,
                ))
            }
        }
    }

    pub fn prop_application(
        &mut self,
        info: &PropInfo,
        arguments: &[ast::Expr],
        span: Span,
    ) -> Elab<Term> {
        if arguments.len() != info.params.len() {
            let message = format!(
                "`{}` takes {} argument(s), and {} were given",
                info.name,
                info.params.len(),
                arguments.len()
            );
            return self.fail("L0208", message, span);
        }
        let mut terms = Vec::new();
        for (argument, param) in arguments.iter().zip(&info.params) {
            let value = self.check(argument, &param.ty)?;
            terms.push(self.term(&value, argument.span)?);
        }
        Ok(Term::PropApp(info.id, terms))
    }

    fn method(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Name,
        arguments: &[ast::Expr],
        span: Span,
    ) -> Elab<Value> {
        let prim = match name.text.as_str() {
            "wrapping_add" => Prim::WrappingAdd,
            "wrapping_sub" => Prim::WrappingSub,
            other => {
                return self.fail("L0207", format!("unknown method `{other}`"), name.span);
            }
        };
        let receiver = self.check(receiver, &Type::U8)?;
        if arguments.len() != 1 {
            return self.fail("L0208", format!("`{}` takes one argument", name.text), span);
        }
        let argument = self.check(&arguments[0], &Type::U8)?;
        Ok(Value::new(
            Expr::Method {
                prim,
                receiver: Box::new(receiver.expr),
                arguments: vec![argument.expr],
            },
            Type::U8,
        ))
    }
}
