//! `erase`: the typed tree to the erased tree. Trusted.
//!
//! It is a projection that preserves shape (specification section 11). A
//! type becomes its erased type. An expression with no runtime form, which
//! is a proof, a proposition, or a call to a function that was not emitted,
//! becomes its marker. Everything else is erased by recursion, in place: a proof-typed variable stays a variable, and a call
//! to an ordinary function that returns a proof stays a call, because at
//! runtime it returns `Proved`.

use crate::kernel::{Definitions, Type};
use crate::typed::{Binder, Block, EnumItem, Expr, FnItem, FnRef, Pattern, Stmt, StructItem};

use super::tree::{EArm, EBlock, EEnum, EExpr, EFn, EPattern, EStmt, EStruct, EType, EVariant};

pub fn erase_type(ty: &Type) -> EType {
    match ty {
        Type::Bool => EType::Bool,
        Type::U8 => EType::U8,
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
    let eraser = Eraser { definitions };
    Some(EFn {
        reference,
        name: item.name.clone(),
        params: item.params.iter().map(bound).collect(),
        result: erase_type(&item.result),
        body: eraser.block(&item.body),
    })
}

fn bound(binder: &Binder) -> (crate::kernel::VarId, String, EType) {
    (binder.id, binder.name.clone(), erase_type(&binder.ty))
}

struct Eraser<'d> {
    definitions: &'d Definitions,
}

impl Eraser<'_> {
    fn block(&self, block: &Block) -> EBlock {
        EBlock {
            stmts: block.stmts.iter().map(|stmt| self.stmt(stmt)).collect(),
            tail: block.tail.as_deref().map(|tail| Box::new(self.expr(tail))),
        }
    }

    fn stmt(&self, stmt: &Stmt) -> EStmt {
        match stmt {
            Stmt::Let { pattern, value } => EStmt::Let {
                pattern: pattern_of(pattern),
                value: self.expr(value),
            },
            Stmt::Expr(expr) => EStmt::Expr(self.expr(expr)),
        }
    }

    fn all(&self, exprs: &[Expr]) -> Vec<EExpr> {
        exprs.iter().map(|expr| self.expr(expr)).collect()
    }

    fn state(&self, state: &[(Binder, Expr)]) -> Vec<(crate::kernel::VarId, String, EType, EExpr)> {
        state
            .iter()
            .map(|(binder, init)| {
                let (id, name, ty) = bound(binder);
                (id, name, ty, self.expr(init))
            })
            .collect()
    }

    fn expr(&self, expr: &Expr) -> EExpr {
        match expr {
            Expr::Proof(_) => EExpr::Proved,
            Expr::Prop(_) => EExpr::Ghost,
            // The empty match: a marker when it stands for a ghost, a trap
            // when it stands for a value.
            Expr::Absurd { ty, .. } => marker(ty).unwrap_or(EExpr::Trap),
            // A function with no runtime form was not emitted, so a call to
            // it has nothing to call. It is total, so nothing is lost.
            Expr::CallMath { id, ty, .. } if !self.definitions.is_executable(*id) => {
                marker(ty).unwrap_or(EExpr::Trap)
            }
            Expr::Var { id, name, .. } => EExpr::Var {
                id: *id,
                name: name.clone(),
            },
            Expr::Bool(value) => EExpr::Bool(*value),
            Expr::U8(value) => EExpr::U8(*value),
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
            Expr::Compare { op, left, right } => EExpr::Compare {
                op: *op,
                left: Box::new(self.expr(left)),
                right: Box::new(self.expr(right)),
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
                ..
            } => EExpr::If {
                condition: Box::new(self.expr(condition)),
                then_block: self.block(then_block),
                else_block: self.block(else_block),
            },
            Expr::Match {
                scrutinee,
                enum_name,
                arms,
                ..
            } => EExpr::Match {
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
            },
            Expr::Block(block) => EExpr::Block(self.block(block)),
            Expr::Loop {
                state,
                result_ty,
                body,
                ..
            } => EExpr::Loop {
                state: self.state(state),
                result: erase_type(result_ty),
                body: self.block(body),
            },
            Expr::For {
                index,
                lo,
                hi,
                state,
                body,
                ..
            } => EExpr::For {
                index: (index.id, index.name.clone()),
                lo: Box::new(self.expr(lo)),
                hi: Box::new(self.expr(hi)),
                state: self.state(state),
                body: self.block(body),
            },
            Expr::Break(value) => EExpr::Break(Box::new(self.expr(value))),
            Expr::Continue(next) => EExpr::Continue(self.all(next)),
        }
    }
}

/// The marker for a ghost type, or `None` for a type with a runtime form.
fn marker(ty: &Type) -> Option<EExpr> {
    match erase_type(ty) {
        EType::Proved => Some(EExpr::Proved),
        EType::Ghost => Some(EExpr::Ghost),
        _ => None,
    }
}

fn pattern_of(pattern: &Pattern) -> EPattern {
    match pattern {
        Pattern::Bind { binder, .. } => EPattern::Bind {
            id: binder.id,
            name: binder.name.clone(),
        },
        Pattern::Wildcard => EPattern::Wildcard,
        Pattern::Tuple(patterns) => EPattern::Tuple(patterns.iter().map(pattern_of).collect()),
    }
}
