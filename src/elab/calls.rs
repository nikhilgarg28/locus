//! Calls: of a function, a proposition, a wrapping method on a machine
//! integer, and the checking of arguments against a telescope of parameter
//! types.

use crate::ast::{self, ExprKind};
use crate::kernel::{Op, Prim, Term, Type, VarId};
use crate::source::Span;
use crate::typed::{Expr, FnRef};

use super::env::{Elab, Env, FnInfo, Global, PropInfo};
use super::exprs::Value;
use super::literals::untyped_literal;

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
        let arguments: Vec<&ast::Expr> = arguments.iter().collect();
        self.arguments_by_ref(&arguments, ids, tys, what, span)
    }

    /// `arguments`, over the values in any order they were found in: a
    /// variant's fields, given by name.
    pub fn arguments_by_ref(
        &mut self,
        arguments: &[&ast::Expr],
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
            ExprKind::Member { value, name } => self.method(value, name, arguments, expected, span),
            ExprKind::Name(name) if self.lookup(&name.text).is_none() => {
                // A call names a function, or applies a proposition.
                let global = self
                    .values
                    .get(&name.text)
                    .or_else(|| self.types.get(&name.text))
                    .cloned();
                match global {
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
                        self.retired_bare_form(name)
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

    /// The rule that admits a function to a proposition, or to the value of
    /// a constant: it promises `terminates`, `no_panic`, and `no_io`, and
    /// takes no `&mut`. `L0209` names what is missing.
    pub(super) fn admit_to_formula(&mut self, info: &FnInfo, place: &str, span: Span) -> Elab<()> {
        let Some(gap) = info.logical_gap() else {
            return Ok(());
        };
        self.diagnostics.push(
            crate::diagnostic::Diagnostic::error(
                "L0209",
                format!("`{}` cannot appear in {place}: {gap}", info.name),
                span,
            )
            .note("a function appears in a proposition when it promises `terminates`, `no_panic`, and `no_io` and takes no `&mut`, so that mentioning it runs nothing and denotes one value"),
        );
        Err(())
    }

    /// A call keeps the caller's promises only if the callee makes each of
    /// them. `L0232` names the first it does not.
    fn keep_promises(&mut self, info: &FnInfo, span: Span) -> Elab<()> {
        let Some(promise) = super::env::first_broken(self.promises, info.promises) else {
            return Ok(());
        };
        self.diagnostics.push(
            crate::diagnostic::Diagnostic::error(
                "L0232",
                format!(
                    "`{}` promises {} and calls `{}`, which does not",
                    self.item_name,
                    promise.name(),
                    info.name
                ),
                span,
            )
            .note(format!(
                "a promise is never inferred: `{}` keeps {} only if it says so, with `#[{}]` or `#![{}]` at the top of its file",
                info.name,
                promise.name(),
                promise.name(),
                promise.name()
            )),
        );
        Err(())
    }

    pub(super) fn call_fn(
        &mut self,
        info: &FnInfo,
        arguments: &[ast::Expr],
        span: Span,
    ) -> Elab<Value> {
        match self.formula {
            Some(place) => self.admit_to_formula(info, place, span)?,
            None => self.keep_promises(info, span)?,
        }
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
                // A formula admits only functions of the logic, and a
                // function of the logic promises what admits its callees.
                if self.total {
                    return self.internal(
                        format!(
                            "`{}` is an ordinary `fn` called where nothing runs",
                            info.name
                        ),
                        span,
                    );
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

    /// A wrapping method, a row of the table of primitive operations at the
    /// receiver's type. A receiver that is a literal without a suffix takes
    /// its type from the argument, as `3.wrapping_sub(n)` does.
    fn method(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Name,
        arguments: &[ast::Expr],
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        let op = match name.text.as_str() {
            "wrapping_add" => Op::WrappingAdd,
            "wrapping_sub" => Op::WrappingSub,
            "wrapping_mul" => Op::WrappingMul,
            "wrapping_neg" => Op::WrappingNeg,
            other => {
                return self.fail("L0207", format!("unknown method `{other}`"), name.span);
            }
        };
        let takes = op.arity() - 1;
        if arguments.len() != takes {
            let count = if takes == 1 {
                "one argument"
            } else {
                "no argument"
            };
            return self.fail("L0208", format!("`{}` takes {count}", name.text), span);
        }
        let (receiver_value, argument_values) = if untyped_literal(receiver)
            && takes == 1
            && !untyped_literal(&arguments[0])
        {
            let argument = self.infer(&arguments[0])?;
            let receiver = self.check(receiver, &argument.ty.clone())?;
            (receiver, vec![argument])
        } else {
            let receiver = match expected {
                Some(expected) if untyped_literal(receiver) => self.check(receiver, expected)?,
                _ => self.infer(receiver)?,
            };
            let mut values = Vec::new();
            for argument in arguments {
                values.push(self.check(argument, &receiver.ty.clone())?);
            }
            (receiver, values)
        };
        let ty = receiver_value.ty.clone();
        let Some(machine) = ty.as_machine() else {
            let shown = self.show_type(&ty);
            return self.fail(
                "L0207",
                format!(
                    "`{}` is a method of the machine integer types, and this is `{shown}`",
                    name.text
                ),
                name.span,
            );
        };
        if op.row(machine).is_none() {
            return self.fail(
                "L0207",
                format!(
                    "`{}` exists at the signed types only, and this is `{}`",
                    name.text,
                    machine.name()
                ),
                name.span,
            );
        }
        Ok(Value::new(
            Expr::Method {
                prim: Prim::Op(op, machine),
                receiver: Box::new(receiver_value.expr),
                arguments: argument_values
                    .into_iter()
                    .map(|value| value.expr)
                    .collect(),
            },
            ty,
        ))
    }
}
