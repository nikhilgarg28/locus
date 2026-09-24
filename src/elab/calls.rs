//! Calls: of a function, a proposition, a wrapping method on a machine
//! integer, and the checking of arguments against a telescope of parameter
//! types.
//!
//! A parameter passed by `&T` or `&mut T` takes a lent place, `&x` or
//! `&mut x.f`, read as a value (`references.rs`). A call with `&mut`
//! arguments returns, in the logic, the tuple of their new values and the
//! declared result: the call binds that tuple, gives each lent root a new
//! version as an assignment does, and its value is the tuple's last field.

use crate::ast::{self, ExprKind};
use crate::kernel::{Op, Prim, Term, Type, VarId};
use crate::source::Span;
use crate::typed::{Expr, FnRef, Passing};

use super::env::{Elab, Env, FnInfo, Global, PropInfo};
use super::exprs::Value;
use super::literals::untyped_literal;

/// An argument of a call: as written, or elaborated already, which is how
/// the receiver of a method reaches the call when it is not a place.
pub(super) enum Argument<'a> {
    Written(&'a ast::Expr),
    Value(Box<Value>, Span),
}

/// Preserve the logical layout marker, wrapping an expression only once.
pub(super) fn ghost_value(expr: Expr) -> Expr {
    match expr {
        ghost @ Expr::Ghost(_) => ghost,
        other => Expr::Ghost(Box::new(other)),
    }
}

impl Env<'_> {
    /// Checks arguments against a telescope of parameter types, written over
    /// the identities `ids`: each argument's term replaces its parameter in
    /// the types that follow. On return `tys` no longer mentions `ids`. The
    /// values may be in any order they were found in: a variant's fields,
    /// given by name. `ghosts` retains logical source layouts for parameter
    /// types which share their kernel representation with runtime types.
    pub fn arguments_by_ref(
        &mut self,
        arguments: &[&ast::Expr],
        ids: &[VarId],
        tys: &mut [Type],
        ghosts: &[bool],
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
            self.expect_layout(argument, &self.session.binding_layout(ids[index]));
            let ghost = ghosts.get(index).copied().unwrap_or(false);
            let value = self.argument(argument, &tys[index].clone(), ghost)?;
            let term = self.term(&value, argument.span)?;
            for later in tys[index + 1..].iter_mut() {
                *later = later.replace_var(ids[index], &term);
            }
            // The last entry may be a result type.
            exprs.push(value.expr);
        }
        Ok(exprs)
    }

    /// A value for a position of type `ty`. Logical positions retain their
    /// erasure layout; eager runtime effects in argument expressions remain.
    pub(super) fn argument(&mut self, argument: &ast::Expr, ty: &Type, ghost: bool) -> Elab<Value> {
        let value = if (matches!(ty, Type::Int)
            || self.is_natural(ty)
            || ghost && matches!(ty, Type::Bool))
            && !untyped_literal(argument)
        {
            let value = self.infer(argument)?;
            let value = self.logical_value(value, argument.span)?;
            self.coerce(value, ty, argument.span)?
        } else {
            self.check(argument, ty)?
        };
        if !ghost && matches!(ty, Type::Bool) && super::reconcile::is_logical_expr(&value.expr) {
            return self.fail(
                "L0272",
                "a runtime bool position cannot receive a logical Bool",
                argument.span,
            );
        }
        Ok(if ghost {
            Value::new(ghost_value(value.expr), value.ty)
        } else {
            value
        })
    }

    pub(super) fn call(
        &mut self,
        callee: &ast::Expr,
        arguments: &[ast::Expr],
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        if let Some(result) = self.box_constructor(callee, arguments, expected, span) {
            return result;
        }
        if let Some(result) = self.vector_constructor(callee, arguments, expected, span) {
            return result;
        }
        match &callee.kind {
            // `Type::name(..)`: a function of an `impl` block, or a variant.
            ExprKind::Path(path) => match self.path_function(path) {
                Some(info) if info.constant => self.fail(
                    "L0208",
                    "a constant is a value; omit the call parentheses",
                    span,
                ),
                Some(info) => self.call_fn(&info, arguments, span),
                None => self.variant(path, arguments, expected, span),
            },
            ExprKind::Member { value, name } => self.method(value, name, arguments, expected, span),
            ExprKind::Name(name) if self.lookup(&name.text).is_none() => {
                // A call names a function, or applies a proposition.
                let global = self
                    .values
                    .get(&name.text)
                    .or_else(|| self.types.get(&name.text))
                    .cloned();
                match global {
                    Some(Global::Fn(info)) if info.constant => self.fail(
                        "L0208",
                        "a constant is a value; omit the call parentheses",
                        span,
                    ),
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
                        self.bare_form(name)
                    }
                    None => {
                        // A function of an `impl` block is named by its type.
                        let suffix = format!("::{}", name.text);
                        let mut owners: Vec<&str> = self
                            .values
                            .keys()
                            .filter_map(|key| key.strip_suffix(suffix.as_str()))
                            .collect();
                        owners.sort_unstable();
                        let mut diagnostic = crate::diagnostic::Diagnostic::error(
                            "L0204",
                            format!("unknown function `{}`", name.text),
                            name.span,
                        );
                        if let Some(owner) = owners.first() {
                            diagnostic = diagnostic.note(format!(
                                "`{name}` is declared in `impl {owner}`: call it as `{owner}::{name}(..)`, or as `x.{name}(..)` on an `x` of that type when it takes `self`",
                                name = name.text
                            ));
                        }
                        self.diagnostics.push(diagnostic);
                        Err(())
                    }
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
        if info.logical {
            return Ok(());
        }
        let diagnostic = crate::diagnostic::Diagnostic::error("L0209", format!("ordinary fn `{}` cannot appear in {place}", info.name), span)
            .note("write logic fn for pure, total logical computation; runtime promises never make an ordinary function logical");
        self.diagnostics.push(diagnostic);
        Err(())
    }

    /// A call keeps the caller's promises only if the callee makes each of
    /// them. `L0232` names the first it does not.
    fn keep_promises(&mut self, info: &FnInfo, span: Span) -> Elab<()> {
        if info.logical {
            return Ok(());
        }
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

    /// A method of an `impl` block called on a receiver: resolved by the
    /// receiver's type, and elaborated as a call with the receiver first.
    /// A receiver that is a place, `x` or `x.f`, is lent for `&self` and
    /// `&mut self`, as `&x` or `&mut x` would be, and read for `self`,
    /// which moves it unless it is `Copy`; any other receiver is a value
    /// of its own, which a method taking `self` consumes and one taking a
    /// reference cannot lend in this tier.
    fn user_method(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Name,
        arguments: &[ast::Expr],
        span: Span,
    ) -> Elab<Value> {
        use super::mutation::{Access, place_path};
        let is_place =
            place_path(receiver).is_some_and(|(root, _)| self.lookup(&root.text).is_some());
        let (ty, value) = if is_place {
            let (root, parts) = place_path(receiver).expect("a place");
            if let Some(inner) = super::mutation::deref_root(receiver) {
                self.deref_target(inner, receiver.span)?;
            }
            let slot = self.place_slot(root)?;
            let (_, ty) = self.place_steps(slot, &parts, Access::Lend)?;
            (ty, None)
        } else {
            // Resolving a receiver's type must not commit moves, mutations,
            // proof obligations, or helper declarations before its method's
            // mode is known. The isolated probe is discarded completely.
            let logical = if self.needs_receiver_mode() {
                let mut probe = self.clone();
                probe.moves.checked = false;
                crate::store::without_store(|| probe.infer(receiver))
                    .ok()
                    .and_then(|value| probe.method_of(&value.ty, &name.text))
                    .map(|method| method.logical)
            } else {
                None
            };
            let value = match logical {
                Some(true) => self.ghost(|env| env.infer(receiver))?,
                Some(false) => self.runtime_arguments(|env| env.infer(receiver))?,
                None => self.infer(receiver)?,
            };
            (value.ty.clone(), Some(value))
        };
        let Some(info) = self.method_of(&ty, &name.text) else {
            let shown = self.show_type(&ty);
            let qualified = self
                .type_name(&ty)
                .map(|owner| format!("{owner}::{}", name.text));
            if qualified.as_deref() == Some(self.item_name.as_str()) {
                self.diagnostics.push(
                    crate::diagnostic::Diagnostic::error(
                        "L0203",
                        format!("`{}` is defined in terms of itself", self.item_name),
                        name.span,
                    )
                    .note("recursion is not part of the core language; a bounded `for` repeats a step a known number of times"),
                );
                return Err(());
            }
            if qualified.is_some_and(|qualified| self.failed.contains(&qualified)) {
                return Err(());
            }
            let mut diagnostic = crate::diagnostic::Diagnostic::error(
                "L0207",
                format!(
                    "no method named `{}` found for `{shown}` (E0599)",
                    name.text
                ),
                name.span,
            );
            if self.type_name(&ty).is_some() {
                diagnostic = diagnostic.note(format!(
                    "a method is declared in `impl {shown} {{ .. }}` with `self`, `&self`, or `&mut self` as its first parameter"
                ));
            } else if ty.as_machine().is_some() {
                diagnostic = diagnostic.note(
                    "the methods of the machine integer types are `wrapping_add`, `wrapping_sub`, `wrapping_mul`, and, at the signed types, `wrapping_neg`",
                );
            }
            self.diagnostics.push(diagnostic);
            return Err(());
        };
        if !info.receiver {
            self.diagnostics.push(
                crate::diagnostic::Diagnostic::error(
                    "L0207",
                    format!(
                        "`{}` takes no `self`, and is called as `{}(..)`",
                        info.name, info.name
                    ),
                    name.span,
                )
                .note("a function of an `impl` block without a `self` parameter is an associated function, named by its type, as in Rust"),
            );
            return Err(());
        }
        let passing = info.passing.first().copied().unwrap_or_default();
        let receiver = match (value, passing.is_reference()) {
            // A place: lent or read as the parameter is passed.
            (None, true) => ast::Expr {
                span: receiver.span,
                kind: ExprKind::Ref {
                    mutable: passing == Passing::RefMut,
                    expr: Box::new(receiver.clone()),
                },
            },
            (None, false) => receiver.clone(),
            (Some(value), false) => {
                let mut all = vec![Argument::Value(Box::new(value), receiver.span)];
                all.extend(arguments.iter().map(Argument::Written));
                return self.call_fn_with(&info, &all, span);
            }
            (Some(_), true) => {
                let wanted = if passing == Passing::RefMut {
                    "&mut self"
                } else {
                    "&self"
                };
                self.diagnostics.push(
                    crate::diagnostic::Diagnostic::error(
                        "L0261",
                        format!(
                            "`{}` takes `{wanted}`, and the receiver is not a place the call could lend",
                            info.name
                        ),
                        receiver.span,
                    )
                    .note("a reference lasts for one call and lends a place the caller holds, `x` or `x.f`; bind the value with `let` first"),
                );
                return Err(());
            }
        };
        let mut all = vec![Argument::Written(&receiver)];
        all.extend(arguments.iter().map(Argument::Written));
        self.call_fn_with(&info, &all, span)
    }

    pub(super) fn call_fn(
        &mut self,
        info: &FnInfo,
        arguments: &[ast::Expr],
        span: Span,
    ) -> Elab<Value> {
        let arguments: Vec<Argument<'_>> = arguments.iter().map(Argument::Written).collect();
        self.call_fn_with(info, &arguments, span)
    }

    /// `call_fn` over arguments some of which were elaborated already: the
    /// receiver of a method that is not a place.
    pub(super) fn call_fn_with(
        &mut self,
        info: &FnInfo,
        arguments: &[Argument<'_>],
        span: Span,
    ) -> Elab<Value> {
        match self.formula {
            Some(place) => self.admit_to_formula(info, place, span)?,
            None => self.keep_promises(info, span)?,
        }
        let ids: Vec<VarId> = info.params.iter().map(|param| param.id).collect();
        let mut tys: Vec<Type> = info.params.iter().map(|param| param.ty.clone()).collect();
        let ghosts: Vec<bool> = info.params.iter().map(|param| param.ghost).collect();
        tys.push(info.result.clone());
        let what = format!("`{}`", info.name);
        // The result type rides along so that it is instantiated too.
        let count = ids.len();
        if arguments.len() != count {
            let given = arguments.len();
            let message = format!(
                "{what} takes {} value{}, and {} {} given",
                count,
                if count == 1 { "" } else { "s" },
                given,
                if given == 1 { "was" } else { "were" },
            );
            return self.fail("L0208", message, span);
        }
        // The arguments of a call erasure removes are read, not moved
        // (`moves.rs`). They are not a logic-only context: a call in them
        // stays, as `let _ = argument;` before the marker, since removing
        // the callee removes nothing an argument does (`erase.rs`).
        let erased = match info.reference {
            FnRef::Math(id) => {
                info.logical || !self.session.program().definitions().is_executable(id)
            }
            FnRef::Exec(_) => false,
        };
        let mut exprs = Vec::new();
        let mut places = Vec::new();
        let mut lent = Vec::new();
        for (index, argument) in arguments.iter().enumerate() {
            let expected = tys[index].clone();
            let passing = info.passing.get(index).copied().unwrap_or_default();
            let (value, argument_span) = match argument {
                // A value elaborated already, which is no place.
                Argument::Value(value, span) => {
                    let value = self.coerce(
                        Value::new(value.expr.clone(), value.ty.clone()),
                        &expected,
                        *span,
                    )?;
                    places.push(None);
                    (value, *span)
                }
                Argument::Written(argument) if passing.is_reference() => {
                    let (value, place) = self.lend_argument(argument, passing, &expected, info)?;
                    if !super::layout::borrow_compatible(
                        &expected,
                        &self.session.expression_layout(&value.expr),
                        &self.session.binding_layout(ids[index]),
                    ) {
                        return self.fail("L0272", "a borrowed argument must have the parameter's logical and runtime positions", argument.span);
                    }
                    if passing == Passing::RefMut {
                        lent.push((index, place.slot, place.steps.clone()));
                    }
                    places.push(Some(place));
                    (value, argument.span)
                }
                Argument::Written(argument) => {
                    self.expect_layout(argument, &self.session.binding_layout(ids[index]));
                    let value = if erased {
                        self.ghost(|env| env.argument(argument, &expected, ghosts[index]))?
                    } else {
                        self.runtime_arguments(|env| {
                            env.argument(argument, &expected, ghosts[index])
                        })?
                    };
                    places.push(self.argument_place(argument, &value));
                    (value, argument.span)
                }
            };
            let term = self.term(&value, argument_span)?;
            for later in tys[index + 1..].iter_mut() {
                *later = later.replace_var(ids[index], &term);
            }
            exprs.push(value.expr);
        }
        self.disjoint_arguments(&places, &exprs)?;
        let ty = tys.pop().expect("the result type was pushed");
        match info.reference {
            FnRef::Math(id) => {
                let expr = Expr::CallMath {
                    id,
                    name: info.name.clone(),
                    arguments: exprs,
                    ty: ty.clone(),
                };
                Ok(Value::new(
                    if info.result_logical && !ty.is_ghost() {
                        ghost_value(expr)
                    } else {
                        expr
                    },
                    ty,
                ))
            }
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
                // A call with `&mut` arguments assigns their roots from the
                // tuple it returns, and its value is the tuple's last field.
                let (lends, ty) = self.lend_write_backs(result, &lent, &ty, span)?;
                let expr = Expr::CallFn {
                    id,
                    name: info.name.clone(),
                    arguments: exprs,
                    result,
                    ty: ty.clone(),
                    lends,
                };
                Ok(Value::new(
                    if info.result_logical && !ty.is_ghost() {
                        ghost_value(expr)
                    } else {
                        expr
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
            // A proposition's arguments are a logic-only context, in a
            // formula or out of one: nothing in them runs, and a name in
            // them is read, not moved (`moves.rs`).
            let value = if self.formula.is_some() {
                self.ghost(|env| env.check(argument, &param.ty))?
            } else {
                self.logical("the arguments of a proposition", |env| {
                    env.check(argument, &param.ty)
                })?
            };
            terms.push(self.term(&value, argument.span)?);
        }
        Ok(Term::PropApp(info.id, terms))
    }

    /// `receiver.name(arguments)`: a wrapping method, a row of the table of
    /// primitive operations at the receiver's type, or a method of an
    /// `impl` block (O4). A receiver of a wrapping method that is a literal
    /// without a suffix takes its type from the argument, as
    /// `3.wrapping_sub(n)` does.
    fn method(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Name,
        arguments: &[ast::Expr],
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        if let Some(result) = self.collection_method(receiver, name, arguments, span) {
            return result;
        }
        let op = match name.text.as_str() {
            "wrapping_add" => Op::WrappingAdd,
            "wrapping_sub" => Op::WrappingSub,
            "wrapping_mul" => Op::WrappingMul,
            "wrapping_neg" => Op::WrappingNeg,
            _ => return self.user_method(receiver, name, arguments, span),
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
        // Wrapping primitives have a checked logical interpretation at the
        // original machine type. Observe their result after resolving the
        // primitive; do not erase the receiver's width first.
        self.suppress_models += 1;
        let values = (|| {
            let values =
                if untyped_literal(receiver) && takes == 1 && !untyped_literal(&arguments[0]) {
                    let argument = self.infer(&arguments[0])?;
                    let receiver = self.check(receiver, &argument.ty.clone())?;
                    (receiver, vec![argument])
                } else {
                    let receiver = match expected {
                        Some(expected) if untyped_literal(receiver) => {
                            self.check(receiver, expected)?
                        }
                        _ => self.infer(receiver)?,
                    };
                    let mut values = Vec::new();
                    for argument in arguments {
                        values.push(self.check(argument, &receiver.ty.clone())?);
                    }
                    (receiver, values)
                };
            Ok(values)
        })();
        self.suppress_models -= 1;
        let (receiver_value, argument_values) = values?;
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
