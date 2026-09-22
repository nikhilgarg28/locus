//! `erase`: the typed tree to the erased tree. Trusted.
//!
//! It is a projection that preserves shape (specification section 11). A
//! type becomes its erased type. An expression with no runtime form, which
//! is a proof, a proposition, or a call to a function that was not emitted,
//! becomes its marker. Everything else is erased by recursion, in place: a proof-typed variable stays a variable, and a call
//! to an ordinary function that returns a proof stays a call, because at
//! runtime it returns `Proved`.
//!
//! The versions lowering gives a mutable binding have no runtime form
//! either: an assignment stays an assignment, and every mention of a version
//! becomes a mention of the binding, which the assignment updated in place.
//! `mut` is printed on a binding only when the function assigns to it.

use std::collections::{HashMap, HashSet};

use crate::kernel::{Definitions, MachineInt, Type, VarId};
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

pub fn erase_struct(id: crate::kernel::StructId, item: &StructItem) -> EStruct {
    EStruct {
        id,
        name: item.name.clone(),
        fields: item
            .fields
            .iter()
            .map(|field| (field.name.clone(), erase_type(&field.ty)))
            .collect(),
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
                payload: variant.payload.iter().map(|b| erase_type(&b.ty)).collect(),
                fields: variant
                    .named
                    .then(|| variant.payload.iter().map(|b| b.name.clone()).collect()),
            })
            .collect(),
    }
}

/// A function's erasure, or `None` when it has no runtime form: a math
/// function that is logical-only, such as a lemma or a predicate.
pub fn erase_fn(definitions: &Definitions, reference: FnRef, item: &FnItem) -> Option<EFn> {
    if let FnRef::Math(id) = reference
        && !definitions.is_executable(id)
    {
        return None;
    }
    let mut eraser = Eraser {
        definitions,
        binding: HashMap::new(),
        assigned: assigned_bindings(&item.body),
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
    (binder.id, binder.name.clone(), erase_type(&binder.ty))
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

struct Eraser<'d> {
    definitions: &'d Definitions,
    /// The binding each version of a mutable binding belongs to, filled as
    /// the versions are met, which is before any mention of them.
    binding: HashMap<VarId, VarId>,
    assigned: HashSet<VarId>,
}

impl Eraser<'_> {
    /// The binding a mention refers to: itself, unless it is a version.
    fn root(&self, id: VarId) -> VarId {
        self.binding.get(&id).copied().unwrap_or(id)
    }

    fn joined(&mut self, joined: Option<&Joined>) {
        for join in joined.iter().flat_map(|joined| &joined.joins) {
            self.binding.insert(join.version.id, join.binding);
        }
    }

    fn block(&mut self, block: &Block) -> EBlock {
        EBlock {
            stmts: block.stmts.iter().map(|stmt| self.stmt(stmt)).collect(),
            tail: block.tail.as_deref().map(|tail| Box::new(self.expr(tail))),
        }
    }

    fn stmt(&mut self, stmt: &Stmt) -> EStmt {
        match stmt {
            Stmt::Let { pattern, value } => EStmt::Let {
                pattern: self.pattern(pattern),
                value: self.expr(value),
            },
            Stmt::Assign {
                place,
                value,
                version,
                ..
            } => {
                let value = self.expr(value);
                self.binding.insert(version.id, place.binding);
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
        }
    }

    fn pattern(&self, pattern: &Pattern) -> EPattern {
        match pattern {
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
            // A function with no runtime form was not emitted, so a call to
            // it has nothing to call. It is total, so nothing of it is lost;
            // an argument that assigns, calls, or loops still runs, in
            // order, before the marker stands for the call.
            Expr::CallMath {
                id, arguments, ty, ..
            } if !self.definitions.is_executable(*id) => {
                let marker = marker(ty).unwrap_or(EExpr::Trap);
                let effects: Vec<EStmt> = arguments
                    .iter()
                    .filter(|argument| has_effects(argument))
                    .map(|argument| EStmt::Let {
                        pattern: EPattern::Wildcard,
                        value: self.expr(argument),
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
            Expr::Var { id, name, .. } => EExpr::Var {
                id: self.root(*id),
                name: name.clone(),
            },
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
                ..
            } => EExpr::Call {
                callee: FnRef::Math(*id),
                name: name.clone(),
                arguments: self.all(arguments),
            },
            Expr::CallFn {
                id,
                name,
                arguments,
                ..
            } => EExpr::Call {
                callee: FnRef::Exec(*id),
                name: name.clone(),
                arguments: self.all(arguments),
            },
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
                let erased = EExpr::Match {
                    scrutinee: Box::new(self.expr(scrutinee)),
                    enum_name: enum_name.clone(),
                    arms: arms
                        .iter()
                        .map(|arm| EArm {
                            variant_name: arm.variant_name.clone(),
                            payload: arm
                                .payload
                                .iter()
                                .map(|binder| (binder.id, binder.name.clone()))
                                .collect(),
                            body: self.block(&arm.body),
                        })
                        .collect(),
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
        }
    }
}

/// Whether evaluating the expression does something that must still
/// happen when its value is not needed: an assignment, a call to an
/// ordinary function, an operator that may panic, a loop, or a transfer
/// of control anywhere inside it.
fn has_effects(expr: &Expr) -> bool {
    let mut found = false;
    each_expr(expr, &mut |expr| {
        found |= matches!(
            expr,
            Expr::CallFn { .. }
                | Expr::Loop { .. }
                | Expr::While { .. }
                | Expr::For { .. }
                | Expr::Break(_)
                | Expr::Continue
                | Expr::Operate { .. }
        );
    });
    each_stmt_under(expr, &mut |stmt| {
        found |= matches!(stmt, Stmt::Assign { .. })
    });
    found
}

/// The marker for a ghost type, or `None` for a type with a runtime form.
fn marker(ty: &Type) -> Option<EExpr> {
    match erase_type(ty) {
        EType::Proved => Some(EExpr::Proved),
        EType::Ghost => Some(EExpr::Ghost),
        _ => None,
    }
}
