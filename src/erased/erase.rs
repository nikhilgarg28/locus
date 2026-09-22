//! `erase`: the typed tree to the erased tree. Trusted.
//!
//! It is a projection that preserves shape (specification section 11). A
//! type becomes its erased type. An expression with no runtime form, which
//! is a proof, a proposition, a `Ghost<T>` value, or a call erasure
//! removes, becomes its marker. Everything else is erased by recursion, in
//! place: a proof-typed variable stays a variable, and a call to an
//! ordinary function that returns a proof stays a call, because at runtime
//! it returns `Proved`.
//!
//! Two rules, which must not be confused (`Eraser::call`):
//!
//! - An erased result. A position is erased exactly when its type has no
//!   runtime form. The evidence, proposition, `Int`, or `Ghost<T>` a call
//!   returns vanishes, and the call stays: it may panic, loop, or do I/O,
//!   and the generated Rust must do the same.
//! - An erasable computation. A call is removed only when its result has
//!   no runtime form and the callee promises `terminates`, `no_panic`, and
//!   `no_io`, and takes no `&mut` to runtime storage: nothing it does is
//!   observable, so leaving it out changes nothing. A kernel function is
//!   such a callee by construction, whether or not it was emitted; an
//!   ordinary function is one by its checked promises. An argument of a
//!   removed call that still does something, an assignment, a call that
//!   is not erasable, an operator that may panic, or a loop, is kept as a
//!   `let _ = argument;` before the marker.
//!
//! A binding with no runtime form, declared `Ghost<T>` or of type `Prop`
//! or `Int`, is not bound: a mention of it is the marker, and its `let`
//! is left out, or kept as `let _ = value;` when the value still does
//! something. A parameter or a field of such a type is the marker, as
//! evidence is. The erased check refuses a `let` of a marker-typed value,
//! so that this rule is judged twice (`check.rs`).
//!
//! The versions lowering gives a mutable binding have no runtime form
//! either: an assignment stays an assignment, and every mention of a version
//! becomes a mention of the binding, which the assignment updated in place.
//! `mut` is printed on a binding only when the function assigns to it.

use std::collections::{HashMap, HashSet};

use crate::exec::Program;
use crate::kernel::{MachineInt, Type, VarId};
use crate::typed::{
    Binder, Block, Carried, EnumItem, Expr, FnItem, FnRef, Joined, Pattern, Stmt, StructItem,
    each_expr, each_stmt, each_stmt_under,
};

use super::tree::{
    EArm, EBlock, EEnum, EExpr, EFn, EPattern, EPlace, EStmt, EStruct, EType, EVariant,
};

pub fn erase_type(ty: &Type) -> EType {
    match ty {
        Type::Bool => EType::Bool,
        Type::U8 => EType::Int(crate::kernel::MachineInt::U8),
        Type::Machine(ty) => EType::Int(*ty),
        Type::Proof(_) => EType::Proved,
        Type::Prop | Type::Nat | Type::Int => EType::Ghost,
        Type::Tuple(fields) => EType::Tuple(fields.iter().map(erase_type).collect()),
        Type::Struct(id) => EType::Struct(*id),
        Type::Enum(id) => EType::Enum(*id),
        // A function into a ghost type is a proof or a predicate.
        Type::Fn(_, result) if matches!(**result, Type::Proof(_)) => EType::Proved,
        Type::Fn(_, result) if result.is_ghost() => EType::Ghost,
        Type::Fn(params, result) => EType::Fn(
            params.iter().map(erase_type).collect(),
            Box::new(erase_type(result)),
        ),
    }
}

/// The erased type of a binder: the marker for one declared `Ghost<T>`,
/// and its type's erasure otherwise.
fn binder_type(binder: &Binder) -> EType {
    if binder.ghost {
        EType::Ghost
    } else {
        erase_type(&binder.ty)
    }
}

/// Whether a binding has no runtime form: declared `Ghost<T>`, or of a
/// type whose erasure is the `Ghost` marker, `Prop` or `Int`. Evidence is
/// not among them: a proof-typed variable stays a variable of `Proved`.
fn vanishes(binder: &Binder) -> bool {
    binder_type(binder) == EType::Ghost
}

pub fn erase_struct(id: crate::kernel::StructId, item: &StructItem) -> EStruct {
    EStruct {
        id,
        name: item.name.clone(),
        fields: item
            .fields
            .iter()
            .map(|field| (field.name.clone(), binder_type(field)))
            .collect(),
        derives: item.derives.clone(),
    }
}

pub fn erase_enum(id: crate::kernel::EnumId, item: &EnumItem) -> EEnum {
    EEnum {
        id,
        name: item.name.clone(),
        variants: item
            .variants
            .iter()
            .map(|variant| EVariant {
                name: variant.name.clone(),
                payload: variant.payload.iter().map(binder_type).collect(),
                fields: variant
                    .named
                    .then(|| variant.payload.iter().map(|b| b.name.clone()).collect()),
            })
            .collect(),
        derives: item.derives.clone(),
    }
}

/// A function's erasure, or `None` when it has no runtime form: a math
/// function that is logical-only, such as a lemma or a predicate. The
/// program is where the promises of the functions it calls are read.
pub fn erase_fn(program: &Program, reference: FnRef, item: &FnItem) -> Option<EFn> {
    if let FnRef::Math(id) = reference
        && !program.definitions().is_executable(id)
    {
        return None;
    }
    let mut eraser = Eraser {
        program,
        binding: HashMap::new(),
        assigned: assigned_bindings(&item.body),
        ghost: item
            .params
            .iter()
            .filter(|param| vanishes(param))
            .map(|param| param.id)
            .collect(),
    };
    Some(EFn {
        reference,
        name: item.name.clone(),
        constant: false,
        params: item.params.iter().map(bound).collect(),
        result: erase_type(&item.result),
        body: eraser.block(&item.body),
    })
}

fn bound(binder: &Binder) -> (VarId, String, EType) {
    (binder.id, binder.name.clone(), binder_type(binder))
}

/// The bindings the body assigns to, whole or by a field: the ones whose
/// `let mut` needs its `mut`.
fn assigned_bindings(body: &Block) -> HashSet<VarId> {
    let mut assigned = HashSet::new();
    each_stmt(body, &mut |stmt| {
        if let Stmt::Assign { place, .. } = stmt {
            assigned.insert(place.binding);
        }
    });
    assigned
}

struct Eraser<'p> {
    program: &'p Program,
    /// The binding each version of a mutable binding belongs to, filled as
    /// the versions are met, which is before any mention of them.
    binding: HashMap<VarId, VarId>,
    assigned: HashSet<VarId>,
    /// The bindings with no runtime form (`vanishes`), filled as they are
    /// met: a mention of one is the marker, and nothing binds it.
    ghost: HashSet<VarId>,
}

impl Eraser<'_> {
    /// The binding a mention refers to: itself, unless it is a version.
    fn root(&self, id: VarId) -> VarId {
        self.binding.get(&id).copied().unwrap_or(id)
    }

    /// The erasable-computation rule: whether a call of `callee` may be
    /// removed once its result is not needed. A kernel function is total
    /// and without effects by construction. An ordinary function is
    /// erasable by its promises, which the exec checker enforced:
    /// `terminates`, `no_panic`, and `no_io`. It must also take no `&mut`
    /// to runtime storage; no function takes `&mut` yet (O3), and when
    /// one can, its signature is read here too.
    fn erasable(&self, callee: FnRef) -> bool {
        match callee {
            FnRef::Math(_) => true,
            FnRef::Exec(id) => self
                .program
                .promises(id)
                .is_some_and(|promises| promises.terminates && promises.no_panic && promises.no_io),
        }
    }

    /// Whether evaluating the expression does something that must still
    /// happen when its value is not needed: an assignment, a call that is
    /// not an erasable computation, an operator that may panic, a loop, a
    /// transfer of control, or a form that panics anywhere inside it.
    fn has_effects(&self, expr: &Expr) -> bool {
        let mut found = false;
        each_expr(expr, &mut |expr| {
            found |= match expr {
                Expr::CallFn { id, .. } => !self.erasable(FnRef::Exec(*id)),
                Expr::Loop { .. }
                | Expr::While { .. }
                | Expr::For { .. }
                | Expr::Break(_)
                | Expr::Continue
                | Expr::Operate { .. }
                | Expr::Panic { .. }
                | Expr::Assert { .. } => true,
                _ => false,
            };
        });
        each_stmt_under(expr, &mut |stmt| {
            found |= matches!(stmt, Stmt::Assign { .. })
        });
        found
    }

    /// The marker in place of values that are not needed, after whatever
    /// the expressions still do, in order.
    fn effects_then(&mut self, exprs: &[Expr], marker: EExpr) -> EExpr {
        let effectful: Vec<&Expr> = exprs.iter().filter(|expr| self.has_effects(expr)).collect();
        let effects: Vec<EStmt> = effectful
            .into_iter()
            .map(|expr| EStmt::Let {
                pattern: EPattern::Wildcard,
                value: self.expr(expr),
            })
            .collect();
        if effects.is_empty() {
            marker
        } else {
            EExpr::Block(EBlock {
                stmts: effects,
                tail: Some(Box::new(marker)),
            })
        }
    }

    /// A call, by the two rules. Removed, with its arguments' effects kept,
    /// when its result has no runtime form and the callee is an erasable
    /// computation, and when the callee was not emitted, which is a kernel
    /// function that is logical-only. Kept otherwise, and its result, when
    /// it has no runtime form, is the marker the callee returns.
    fn call(&mut self, callee: FnRef, name: &str, arguments: &[Expr], ty: &Type) -> EExpr {
        let emitted = match callee {
            FnRef::Math(id) => self.program.definitions().is_executable(id),
            FnRef::Exec(_) => true,
        };
        let result = marker(ty);
        if !emitted || (result.is_some() && self.erasable(callee)) {
            // A function that was not emitted has a ghost result, so the
            // trap stands only for a call the checker never accepts.
            let marker = result.unwrap_or(EExpr::Trap);
            return self.effects_then(arguments, marker);
        }
        EExpr::Call {
            callee,
            name: name.to_string(),
            arguments: self.all(arguments),
        }
    }

    fn joined(&mut self, joined: Option<&Joined>) {
        for join in joined.iter().flat_map(|joined| &joined.joins) {
            self.binding.insert(join.version.id, join.binding);
        }
    }

    fn block(&mut self, block: &Block) -> EBlock {
        EBlock {
            stmts: block
                .stmts
                .iter()
                .filter_map(|stmt| self.stmt(stmt))
                .collect(),
            tail: block.tail.as_deref().map(|tail| Box::new(self.expr(tail))),
        }
    }

    /// A statement, or `None` for one that erases to nothing: a `let` that
    /// binds nothing with a runtime form to a bare marker, or an assignment
    /// of a bare marker to a binding that has no runtime form.
    fn stmt(&mut self, stmt: &Stmt) -> Option<EStmt> {
        Some(match stmt {
            Stmt::Let { pattern, value } => {
                let value = self.expr(value);
                let pattern = self.pattern(pattern);
                if matches!(pattern, EPattern::Wildcard) && is_marker(&value) {
                    return None;
                }
                EStmt::Let { pattern, value }
            }
            Stmt::Assign {
                place,
                value,
                version,
                ..
            } => {
                let value = self.expr(value);
                self.binding.insert(version.id, place.binding);
                if self.ghost.contains(&place.binding) {
                    if is_marker(&value) {
                        return None;
                    }
                    return Some(EStmt::Let {
                        pattern: EPattern::Wildcard,
                        value,
                    });
                }
                EStmt::Assign {
                    place: EPlace {
                        id: place.binding,
                        name: place.name.clone(),
                        path: place
                            .path
                            .iter()
                            .map(|step| (step.index, step.name.clone()))
                            .collect(),
                    },
                    value,
                }
            }
            Stmt::Expr(expr) => EStmt::Expr(self.expr(expr)),
        })
    }

    /// A `let` pattern. A name with no runtime form binds nothing: it is
    /// recorded, so that a mention of it is the marker, and it stands as
    /// `_` here.
    fn pattern(&mut self, pattern: &Pattern) -> EPattern {
        match pattern {
            Pattern::Bind { binder, .. } if vanishes(binder) => {
                self.ghost.insert(binder.id);
                EPattern::Wildcard
            }
            Pattern::Bind {
                binder, mutable, ..
            } => EPattern::Bind {
                id: binder.id,
                name: binder.name.clone(),
                ty: erase_type(&binder.ty),
                mutable: *mutable && self.assigned.contains(&binder.id),
            },
            Pattern::Wildcard => EPattern::Wildcard,
            Pattern::Tuple(patterns) => {
                EPattern::Tuple(patterns.iter().map(|part| self.pattern(part)).collect())
            }
        }
    }

    fn all(&mut self, exprs: &[Expr]) -> Vec<EExpr> {
        exprs.iter().map(|expr| self.expr(expr)).collect()
    }

    /// The versions a loop's body sees of what it carries, and the versions
    /// after it, are the bindings, which the body assigns in place.
    fn carried(&mut self, state: &[Binder], carried: &Carried) {
        for (inside, join) in state.iter().zip(&carried.joins) {
            self.binding.insert(inside.id, join.binding);
            self.binding.insert(join.version.id, join.binding);
        }
    }

    fn expr(&mut self, expr: &Expr) -> EExpr {
        match expr {
            Expr::Proof(_) => EExpr::Proved,
            Expr::Prop(_) | Expr::Int(_) | Expr::IntArith { .. } => EExpr::Ghost,
            // The evidence and the learned facts are logical; the operation
            // stays the operator it is, at its type.
            Expr::Operate {
                op, ty, operands, ..
            } => EExpr::Operate {
                op: *op,
                ty: *ty,
                operands: self.all(operands),
            },
            // The empty match: a marker when it stands for a ghost, a trap
            // when it stands for a value.
            Expr::Absurd { ty, .. } => marker(ty).unwrap_or(EExpr::Trap),
            // A `Ghost<T>` value is the marker. What stands inside it was
            // elaborated where nothing runs, so it does nothing; should it
            // still, that is kept, as for the arguments of a removed call.
            Expr::Ghost(inner) => self.effects_then(std::slice::from_ref(inner), EExpr::Ghost),
            Expr::Var { id, name, .. } => {
                let id = self.root(*id);
                if self.ghost.contains(&id) {
                    EExpr::Ghost
                } else {
                    EExpr::Var {
                        id,
                        name: name.clone(),
                    }
                }
            }
            Expr::Bool(value) => EExpr::Bool(*value),
            Expr::Literal(ty, value) => EExpr::Literal(*ty, *value),
            Expr::Tuple { fields, .. } => EExpr::Tuple(self.all(fields)),
            Expr::Struct { id, name, fields } => EExpr::Struct {
                id: *id,
                name: name.clone(),
                fields: fields
                    .iter()
                    .map(|(field, value)| (field.clone(), self.expr(value)))
                    .collect(),
            },
            Expr::Variant {
                id,
                enum_name,
                index,
                variant_name,
                payload,
            } => EExpr::Variant {
                id: *id,
                enum_name: enum_name.clone(),
                index: *index,
                variant_name: variant_name.clone(),
                payload: self.all(payload),
            },
            Expr::Field {
                target,
                index,
                name,
                ..
            } => EExpr::Field {
                target: Box::new(self.expr(target)),
                index: *index,
                name: name.clone(),
            },
            Expr::Method {
                prim,
                receiver,
                arguments,
            } => EExpr::Method {
                prim: *prim,
                receiver: Box::new(self.expr(receiver)),
                arguments: self.all(arguments),
            },
            Expr::Compare {
                op, left, right, ..
            } => EExpr::Compare {
                op: *op,
                left: Box::new(self.expr(left)),
                right: Box::new(self.expr(right)),
            },
            // `as Int` is ghost. `Int as T` is not, but its argument is, so
            // it stands only in a function with no runtime form, which is
            // never erased; the marker is what an erasure of it would be.
            Expr::Cast { expr, from, to } => match (from.as_machine(), to.as_machine()) {
                (Some(_), Some(to)) => EExpr::Cast {
                    expr: Box::new(self.expr(expr)),
                    to,
                },
                _ => EExpr::Ghost,
            },
            Expr::CallMath {
                id,
                name,
                arguments,
                ty,
            } => self.call(FnRef::Math(*id), name, arguments, ty),
            Expr::CallFn {
                id,
                name,
                arguments,
                ty,
                ..
            } => self.call(FnRef::Exec(*id), name, arguments, ty),
            Expr::If {
                condition,
                then_block,
                else_block,
                joined,
                ..
            } => {
                let erased = EExpr::If {
                    condition: Box::new(self.expr(condition)),
                    then_block: self.block(then_block),
                    else_block: self.block(else_block),
                };
                self.joined(joined.as_ref());
                erased
            }
            Expr::Match {
                scrutinee,
                enum_name,
                arms,
                joined,
                ..
            } => {
                let scrutinee = Box::new(self.expr(scrutinee));
                let mut erased_arms = Vec::new();
                for arm in arms {
                    // A payload with no runtime form is bound to the marker
                    // the variant holds; a mention of it is the marker.
                    for binder in arm.payload.iter().filter(|binder| vanishes(binder)) {
                        self.ghost.insert(binder.id);
                    }
                    erased_arms.push(EArm {
                        variant_name: arm.variant_name.clone(),
                        payload: arm
                            .payload
                            .iter()
                            .map(|binder| (binder.id, binder.name.clone()))
                            .collect(),
                        body: self.block(&arm.body),
                    });
                }
                let erased = EExpr::Match {
                    scrutinee,
                    enum_name: enum_name.clone(),
                    arms: erased_arms,
                };
                self.joined(joined.as_ref());
                erased
            }
            Expr::Block(block) => EExpr::Block(self.block(block)),
            Expr::Loop {
                state,
                carried,
                ty,
                body,
                ..
            } => {
                self.carried(state, carried);
                EExpr::Loop {
                    result: erase_type(ty),
                    body: self.block(body),
                }
            }
            Expr::While {
                condition,
                state,
                carried,
                body,
                ..
            } => {
                self.carried(state, carried);
                EExpr::While {
                    condition: Box::new(self.expr(condition)),
                    body: self.block(body),
                }
            }
            Expr::For {
                index,
                lo,
                hi,
                inclusive,
                state,
                carried,
                body,
                ..
            } => {
                self.carried(state, carried);
                let ty = index.ty.as_machine().unwrap_or(MachineInt::U8);
                EExpr::For {
                    index: (index.id, index.name.clone(), ty),
                    lo: Box::new(self.expr(lo)),
                    hi: Box::new(self.expr(hi)),
                    inclusive: *inclusive,
                    body: self.block(body),
                }
            }
            Expr::Break(value) => {
                EExpr::Break(value.as_deref().map(|value| Box::new(self.expr(value))))
            }
            Expr::Continue => EExpr::Continue,
            // The evidence that a panic is unreachable, or that a check
            // passes, is logical; the form stays as it was written.
            Expr::Panic { form, argument, .. } => EExpr::Panic {
                form: *form,
                argument: argument.clone(),
            },
            Expr::Assert {
                debug,
                condition,
                message,
                ..
            } => EExpr::Assert {
                debug: *debug,
                condition: Box::new(self.expr(condition)),
                message: message.clone(),
            },
        }
    }
}

/// A bare marker: nothing of it runs.
fn is_marker(expr: &EExpr) -> bool {
    matches!(expr, EExpr::Proved | EExpr::Ghost)
}

/// The marker for a ghost type, or `None` for a type with a runtime form.
fn marker(ty: &Type) -> Option<EExpr> {
    match erase_type(ty) {
        EType::Proved => Some(EExpr::Proved),
        EType::Ghost => Some(EExpr::Ghost),
        _ => None,
    }
}
