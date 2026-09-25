//! References as parameters (O3): `&T` and `&mut T` as parameter types,
//! arguments written as paths, `old!`, and the rule that keeps the
//! arguments of one call apart. Tier 0 of references, and nothing more: a
//! reference lasts for one call, so it is written on a parameter and on an
//! argument, and nowhere else.
//!
//! A `&T` parameter is the value lent: the body reads it, and moves nothing
//! out of it (`moves.rs`). A `&mut T` parameter is a mutable binding of
//! the body, as a `let mut` is, and its final version is passed back: in
//! the logic the function takes the entry value and returns the exit value
//! beside its declared result. In the parameter types the name means the
//! entry value; in the result type it means the exit value, for which the
//! elaborator shadows the parameter with an exit binder while the result
//! type is read (`result_over_exits`), and the body's tail is checked
//! against that type with each exit binder replaced by the version current
//! at the end (`at_current_exit`). `old!(x)` names the entry value in a
//! proposition, which is the parameter's own identity, its first version.
//!
//! A call passes a place to a reference parameter, `&x`, `&mut x.f`: the
//! place is read as a value, and not moved; a `&mut` place must be rooted
//! at a mutable binding, and, like an assignment, may not be a field that
//! evidence in the same product depends on. After the call each `&mut`
//! root gets a new version from the tuple the call returns, through the
//! `write_back` of an assignment (`mutation.rs`), so tracked evidence
//! about it is stale until refreshed, and evidence the callee returns
//! about its exit value is what refreshes it. Two arguments of one call
//! may not overlap when either is lent by `&mut`; the messages carry the
//! codes rustc would give, and lowering checks the same rule, trusted.

use std::borrow::Cow;

use crate::ast::{self, ExprKind};
use crate::diagnostic::Diagnostic;
use crate::kernel::{Term, Type, VarId};
use crate::source::Span;
use crate::typed::{Binder, Expr, Lend, Passing, Place, Step, each_expr};

use super::env::{Elab, Env, FnInfo, Local};
use super::exprs::Value;
use super::mutation::{Access, place_path, type_mentions};
use super::types::Written;

/// The place an argument names, for the rule that keeps arguments apart:
/// the local, the path into it, how the argument reaches it, whether its
/// type is `Copy`, and its spelling.
pub(super) struct ArgumentPlace {
    pub slot: usize,
    pub steps: Vec<Step>,
    pub access: Access,
    pub copy: bool,
    pub shown: String,
    pub span: Span,
}

impl ArgumentPlace {
    fn path(&self) -> Vec<usize> {
        self.steps.iter().map(|step| step.index).collect()
    }
}

impl Env<'_> {
    // --- Parameters ---

    /// The type of a parameter, as written, and how it is passed: `&T` and
    /// `&mut T` peel to `T`, and `mut name: T` is passed by value under
    /// `mut`.
    pub(super) fn parameter_type(
        &mut self,
        parameter: &ast::Parameter,
    ) -> Elab<(Written, Passing)> {
        match &parameter.ty.kind {
            ast::TypeKind::Ref { mutable, inner, .. } => {
                if parameter.mutable {
                    return self.fail(
                        "L0260",
                        format!(
                            "`mut {}` is not written on a reference parameter: the value behind `&mut` is assigned through it, and the reference itself is never rebound",
                            parameter.name.text
                        ),
                        parameter.span,
                    );
                }
                let written = if let ast::TypeKind::Slice(element) = &inner.kind {
                    Written {
                        ty: self.collection_type(element, inner.span)?,
                        ghost: false,
                    }
                } else {
                    self.written(inner)?
                };
                Ok((
                    written,
                    if *mutable {
                        Passing::RefMut
                    } else {
                        Passing::Ref
                    },
                ))
            }
            _ => {
                let written = self.written(&parameter.ty)?;
                Ok((
                    written,
                    if parameter.mutable {
                        Passing::MutValue
                    } else {
                        Passing::Value
                    },
                ))
            }
        }
    }

    pub(super) fn observation_parameter(&mut self, parameter: &ast::Parameter) -> Elab<Written> {
        let ty = parameter.ty.observed();
        if parameter.mutable || matches!(ty.kind, ast::TypeKind::Ref { mutable: true, .. }) {
            return self.fail(
                "L0270",
                "logical parameters are read-only observations",
                parameter.span,
            );
        }
        if let ast::TypeKind::Slice(element) = &ty.kind {
            Ok(Written {
                ty: self.collection_type(element, ty.span)?,
                ghost: false,
            })
        } else {
            self.written(ty)
        }
    }

    /// What a parameter's passing means in the body: a `mut` or `&mut`
    /// parameter is a mutable binding, and a reference parameter is one
    /// nothing is moved out of.
    pub(super) fn declare_passing(&mut self, id: VarId, passing: Passing) {
        if passing.is_mutable() {
            self.make_mutable(id);
        }
        if passing.is_reference() {
            self.borrowed.push(id);
        }
    }

    /// The exit binders of the `&mut` parameters, in parameter order: a
    /// fresh binder of each parameter's type, declared in the mirrored
    /// context, which stands for the parameter's value at return.
    pub(super) fn exit_binders(
        &mut self,
        params: &[Binder],
        passing: &[Passing],
        span: Span,
    ) -> Elab<Vec<Binder>> {
        let mut exits = Vec::new();
        for (param, mode) in params.iter().zip(passing) {
            if *mode != Passing::RefMut {
                continue;
            }
            let exit = Binder::new(&param.name, param.ty.clone());
            self.session
                .register_binding_layout(exit.id, self.session.binding_layout(param.id));
            let declared = self.ctx.declare_with(exit.id, exit.ty.clone(), false);
            self.kernel(declared, span)?;
            self.labels.insert(exit.id, param.name.clone());
            self.exits.push((exit.id, param.id, param.name.clone()));
            exits.push(exit);
        }
        Ok(exits)
    }

    /// The result type, read with each `&mut` parameter standing for its
    /// value at return: its exit binder shadows the parameter while the
    /// type is read. Returns the exit binders, in parameter order, and the
    /// type over them.
    pub(super) fn result_over_exits(
        &mut self,
        result: &ast::Type,
        params: &[Binder],
        passing: &[Passing],
    ) -> Elab<(Vec<Binder>, Type)> {
        let exits = self.exit_binders(params, passing, result.span)?;
        let mark = self.names.len();
        for exit in &exits {
            self.names.push(Local {
                name: exit.name.clone(),
                id: exit.id,
                ty: exit.ty.clone(),
                poisoned: false,
                ghost: false,
                binding: None,
                moved: Vec::new(),
                tracked: None,
            });
        }
        let ty = self.ty(result);
        self.names.truncate(mark);
        Ok((exits, ty?))
    }

    /// A type as it is meant here: an exit binder in it, the value of a
    /// `&mut` parameter at return, is the version current at this point.
    pub(super) fn at_current_exit<'t>(&self, ty: &'t Type) -> Cow<'t, Type> {
        let mut ty = Cow::Borrowed(ty);
        for (exit, binding, _) in &self.exits {
            if type_mentions(&ty, *exit)
                && let Some(current) = self.current_version(*binding)
            {
                ty = Cow::Owned(ty.replace_var(*exit, &Term::var(current)));
            }
        }
        ty
    }

    /// `old!(x)`: the value the `&mut` parameter `x` had at entry, in a
    /// proposition.
    pub(super) fn old_form(
        &mut self,
        arguments: &[ast::Expr],
        name_span: Span,
        span: Span,
    ) -> Elab<Value> {
        if self.formula.is_none() {
            self.diagnostics.push(
                Diagnostic::error(
                    "L0264",
                    "`old!` stands in a proposition: a result type, a proof type, `prove!`, or `prop!`",
                    name_span,
                )
                .note("`old!(x)` names the value the `&mut` parameter `x` had when the function was entered; the code has only the value `x` has now"),
            );
            return Err(());
        }
        let [
            ast::Expr {
                kind: ExprKind::Name(name),
                ..
            },
        ] = arguments
        else {
            return self.fail("L0264", "`old!` takes the name of a `&mut` parameter", span);
        };
        let Some((_, binding, _)) = self.exits.iter().find(|(_, _, of)| *of == name.text) else {
            if self.lookup(&name.text).is_none() && !self.failed.contains(&name.text) {
                return self.fail("L0204", format!("unknown name `{}`", name.text), name.span);
            }
            self.diagnostics.push(
                Diagnostic::error(
                    "L0264",
                    format!("`{}` is not a `&mut` parameter, so it has no entry value to name", name.text),
                    name.span,
                )
                .note("`old!(x)` speaks of a parameter `x: &mut T` as it was at entry; any other value is the same value throughout, and is named as it is"),
            );
            return Err(());
        };
        let binding = *binding;
        let ty = self.type_of(&Term::var(binding), span)?;
        Ok(Value::new(
            Expr::Var {
                id: binding,
                name: format!("old!({})", name.text),
                ty: ty.clone(),
            },
            ty,
        ))
    }

    // --- Arguments ---

    /// A reference argument, `&place` or `&mut place`, for a parameter
    /// passed by reference: the place read as a value, and the place.
    pub(super) fn lend_argument(
        &mut self,
        argument: &ast::Expr,
        passing: Passing,
        expected: &Type,
        info: &FnInfo,
    ) -> Elab<(Value, ArgumentPlace)> {
        let mutable = passing == Passing::RefMut;
        let wanted = if mutable { "&mut " } else { "&" };
        let ExprKind::Ref {
            mutable: written,
            expr: inner,
        } = &argument.kind
        else {
            let shown = self.show_type(expected);
            let text = self.spelled_text(argument.span);
            self.diagnostics.push(
                Diagnostic::error(
                    "L0261",
                    format!("`{}` takes `{wanted}{shown}` here; write `{wanted}{text}`", info.name),
                    argument.span,
                )
                .note("a reference parameter takes a lent place, `&x` or `&mut x.f`, which the call reads and, for `&mut`, writes"),
            );
            return Err(());
        };
        if *written != mutable {
            let shown = self.show_type(expected);
            let found = if *written { "&mut " } else { "&" };
            self.diagnostics.push(
                Diagnostic::error(
                    "L0261",
                    format!("expected `{wanted}{shown}`, found `{found}{shown}`"),
                    argument.span,
                )
                .note(format!(
                    "the parameter of `{}` is `{wanted}{shown}`, and a lend is written the way the parameter is",
                    info.name
                )),
            );
            return Err(());
        }
        let Some((root, parts)) = place_path(inner) else {
            self.diagnostics.push(
                Diagnostic::error(
                    "L0261",
                    "a reference argument is a path to a local: `&x` or `&mut x.f`",
                    inner.span,
                )
                .note("a reference lasts for one call, so what it points to is a place the caller holds"),
            );
            return Err(());
        };
        if self.lookup(&root.text).is_none() && self.values.contains_key(&root.text) {
            return self.fail(
                "L0261",
                format!(
                    "a reference argument names a local, and `{}` is an item",
                    root.text
                ),
                root.span,
            );
        }
        let slot = self.place_slot(root)?;
        if mutable && self.names[slot].binding.is_none() {
            let name = root.text.clone();
            let diagnostic = if self.borrowed.contains(&self.names[slot].id) {
                Diagnostic::error(
                    "L0262",
                    format!("cannot borrow `*{name}` as mutable, as it is behind a `&` reference (E0596)"),
                    argument.span,
                )
                .note(format!(
                    "`{name}` is a `&` parameter, read and never written; to write through it, take `{name}: &mut T`"
                ))
            } else {
                Diagnostic::error(
                    "L0262",
                    format!("cannot borrow `{name}` as mutable, as it is not declared as mutable (E0596)"),
                    argument.span,
                )
                .note(format!(
                    "a `&mut` argument is rooted at a binding the call may assign: declare it `let mut {name}`, or as a parameter `mut {name}: T` or `{name}: &mut T`"
                ))
            };
            self.diagnostics.push(diagnostic);
            return Err(());
        }
        let access = if mutable {
            Access::LendMut
        } else {
            Access::Lend
        };
        let (steps, _) = self.place_steps(slot, &parts, access)?;
        // The place read as a value, not moved (`moves.rs`).
        self.suppress_models += 1;
        let value = self.lending(|env| env.check(inner, expected));
        self.suppress_models -= 1;
        let value = value?;
        let local = &self.names[slot];
        let place = Place {
            binding: local.binding.unwrap_or(local.id),
            name: local.name.clone(),
            path: steps.clone(),
        };
        let argument_place = ArgumentPlace {
            slot,
            steps,
            access,
            copy: true,
            shown: self.spelled_text(inner.span),
            span: argument.span,
        };
        Ok((
            Value {
                expr: Expr::Lend {
                    mutable,
                    place,
                    value: Box::new(value.expr),
                },
                ty: value.ty,
                never: false,
            },
            argument_place,
        ))
    }

    /// The place a by-value argument reads, if it is one: `x` or `x.f`.
    pub(super) fn argument_place(
        &self,
        argument: &ast::Expr,
        value: &Value,
    ) -> Option<ArgumentPlace> {
        let (slot, path) = self.place_of(&value.expr)?;
        let steps = path
            .into_iter()
            .map(|index| Step {
                index,
                name: None,
                ty: Type::Tuple(Vec::new()),
                proof_fields: Vec::new(),
            })
            .collect();
        Some(ArgumentPlace {
            slot,
            steps,
            access: Access::Read,
            copy: self.is_copy(&value.ty),
            shown: self.spelled_text(argument.span),
            span: argument.span,
        })
    }

    /// Source text on one line, for a message.
    fn spelled_text(&self, span: Span) -> String {
        self.text(span)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The rule that keeps the arguments of one call apart, as rustc rules
    /// it: every lend lasts until the call returns, and the arguments are
    /// evaluated left to right. Two lends of overlapping places, one path
    /// a prefix of the other at one root, conflict when either is `&mut`
    /// (E0499, E0502); a place read after a `&mut` lend of an overlapping
    /// one is used while borrowed (E0503), and so is a root lent by `&mut`
    /// that a later argument mentions anywhere in it; a value that is not
    /// `Copy` moved out of a place lent by `&` earlier in the call is moved
    /// while borrowed (E0505). A place read before a `&mut` lend was
    /// copied first, and is fine.
    pub(super) fn disjoint_arguments(
        &mut self,
        places: &[Option<ArgumentPlace>],
        arguments: &[Expr],
    ) -> Elab<()> {
        for (i, first) in places.iter().enumerate() {
            let Some(first) = first else {
                continue;
            };
            if first.access == Access::LendMut {
                let root = self.names[first.slot]
                    .binding
                    .unwrap_or(self.names[first.slot].id);
                for (later, place) in arguments.iter().zip(places).skip(i + 1) {
                    // A place named whole by the argument is judged below,
                    // by its path; a mention inside it is a use.
                    if place.as_ref().is_some_and(|place| place.slot == first.slot) {
                        continue;
                    }
                    if let Some(mention) = self.mentions_root(later, root) {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "L0263",
                                format!("cannot use `{mention}` because it was mutably borrowed (E0503)"),
                                first.span,
                            )
                            .note(format!(
                                "`{}` is lent by `&mut` for the whole call, and a later argument mentions it; read it into a `let` before the call, or lend it last",
                                first.shown
                            )),
                        );
                        return Err(());
                    }
                }
            }
            for second in places[i + 1..].iter().flatten() {
                let (a, b) = (first.path(), second.path());
                if first.slot != second.slot || !a.iter().zip(&b).all(|(x, y)| x == y) {
                    continue;
                }
                let conflict = match (first.access, second.access) {
                    (Access::LendMut, Access::LendMut) => Some((
                        "E0499",
                        format!(
                            "cannot borrow `{}` as mutable more than once at a time",
                            second.shown
                        ),
                    )),
                    (Access::LendMut, Access::Lend) => Some((
                        "E0502",
                        format!(
                            "cannot borrow `{}` as immutable because it is also borrowed as mutable",
                            second.shown
                        ),
                    )),
                    (Access::Lend, Access::LendMut) => Some((
                        "E0502",
                        format!(
                            "cannot borrow `{}` as mutable because it is also borrowed as immutable",
                            second.shown
                        ),
                    )),
                    (Access::LendMut, Access::Read) => Some((
                        "E0503",
                        format!(
                            "cannot use `{}` because it was mutably borrowed",
                            second.shown
                        ),
                    )),
                    (Access::Lend, Access::Read) if !second.copy => Some((
                        "E0505",
                        format!(
                            "cannot move out of `{}` because it is borrowed",
                            second.shown
                        ),
                    )),
                    _ => None,
                };
                if let Some((code, message)) = conflict {
                    self.diagnostics.push(
                        Diagnostic::error("L0263", format!("{message} ({code})"), second.span)
                            .label(
                                first.span,
                                format!("`{}` is lent here, for the whole call", first.shown),
                            )
                            .note("the arguments of one call are evaluated left to right, and a lend lasts until the call returns: two arguments may not name overlapping places when either is lent by `&mut`, and a value lent by `&` is not moved, as rustc rules"),
                    );
                    return Err(());
                }
            }
        }
        Ok(())
    }

    /// The name of a local mentioned in a runtime position of the
    /// expression whose binding, or identity, is `root`, if any.
    fn mentions_root(&self, expr: &Expr, root: VarId) -> Option<String> {
        let mut found = None;
        each_expr(expr, &mut |expr| {
            if found.is_some() {
                return;
            }
            let id = match expr {
                Expr::Var { id, .. } => *id,
                Expr::Lend { place, .. } => place.binding,
                _ => return,
            };
            let mentioned = self
                .names
                .iter()
                .rev()
                .find(|local| local.id == id || local.binding == Some(id))
                .map(|local| (local.binding.unwrap_or(local.id), local.name.clone()));
            if let Some((of, name)) = mentioned
                && of == root
            {
                found = Some(name);
            }
        });
        found
    }

    /// After a call with `&mut` arguments, whose tuple result is `result`:
    /// each lent root gets a new version from the tuple's field for it,
    /// in argument order, and the call's value is the tuple's last field.
    /// Returns the lends for the tree and the type of the value.
    pub(super) fn lend_write_backs(
        &mut self,
        result: VarId,
        lent: &[(usize, usize, Vec<Step>)],
        ty: &Type,
        span: Span,
    ) -> Elab<(Vec<Lend>, Type)> {
        if lent.is_empty() {
            return Ok((Vec::new(), ty.clone()));
        }
        let mut lends = Vec::new();
        for (index, (argument, slot, steps)) in lent.iter().enumerate() {
            let returned = Term::proj(Term::var(result), index);
            let (version, equation) =
                self.write_back(*slot, steps, returned, Access::LendMut, span)?;
            lends.push(Lend {
                argument: *argument,
                version,
                equation,
            });
        }
        let value = Term::proj(Term::var(result), lent.len());
        let ty = self.type_of(&value, span)?;
        Ok((lends, ty))
    }
}

impl Env<'_> {
    pub(super) fn shared_reference(
        &mut self,
        inner: &ast::Expr,
        mutable: bool,
        span: Span,
    ) -> Elab<Value> {
        if mutable {
            return self.fail(
                "L0285",
                "mutable references may only be lent for one call",
                span,
            );
        }
        if self.total {
            self.suppress_models += 1;
            let value = self.lending(|env| env.infer(inner));
            self.suppress_models -= 1;
            return value;
        }
        self.require_preview(
            crate::preview::Feature::HeapViews,
            "stored shared references",
            span,
        )?;
        if let ast::ExprKind::Subscript { value, index } = &inner.kind {
            return self.buffer_shared_index(value, index, span);
        }
        let result = self.lending(|env| env.infer(inner));
        let value = result?;
        if self.place_of(&value.expr).is_none()
            && !matches!(value.expr, Expr::Deref(_) | Expr::BoxDeref { .. })
        {
            return self.fail(
                "L0286",
                "a stored shared reference must borrow a local, a field, or an existing reference",
                span,
            );
        }
        Ok(Value::new(
            Expr::Shared {
                value: Box::new(value.expr),
                lifetime: None,
            },
            value.ty,
        ))
    }
}
