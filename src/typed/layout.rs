//! Source erasure layout independent of the kernel's mathematical types.
//!
//! `Bool` and `bool` share one kernel type, but never one runtime layout.
//! Keeping the distinction here also covers nested tuples without adding a
//! second mathematical boolean to the trusted kernel.

use std::collections::HashMap;

use super::{Expr, FnRef};
use crate::kernel::{Definitions, EnumId, StructId, Type, VarId};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ErasureLayout {
    /// Use the type's intrinsic layout (proofs and propositions still erase).
    #[default]
    Default,
    Logical,
    Shared {
        lifetime: Option<String>,
        inner: Box<ErasureLayout>,
    },
    Borrowed {
        lifetime: Option<String>,
        inner: Box<ErasureLayout>,
    },
    NominalLifetimes(Vec<String>),
    Boxed(Box<ErasureLayout>),
    Tuple(Vec<ErasureLayout>),
    Buffer {
        storage: crate::exec::BufferStorage,
        element: Box<ErasureLayout>,
    },
}

impl ErasureLayout {
    pub fn field(&self, index: usize) -> Self {
        match self {
            Self::Logical => Self::Logical,
            Self::Shared { inner, .. } | Self::Borrowed { inner, .. } => inner.field(index),
            Self::NominalLifetimes(_) => Self::Default,
            Self::Boxed(inner) if index == 0 => (**inner).clone(),
            Self::Boxed(_) => Self::Default,
            Self::Tuple(fields) => fields.get(index).cloned().unwrap_or_default(),
            Self::Default | Self::Buffer { .. } => Self::Default,
        }
    }

    pub fn is_logical(&self) -> bool {
        matches!(self, Self::Logical)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ErasureLayouts {
    pub bindings: HashMap<VarId, ErasureLayout>,
    pub functions: HashMap<FnRef, ErasureLayout>,
    pub structs: HashMap<StructId, Vec<ErasureLayout>>,
    pub struct_lifetimes: HashMap<StructId, Vec<String>>,
    pub enum_lifetimes: HashMap<EnumId, Vec<String>>,
    pub enums: HashMap<EnumId, Vec<Vec<ErasureLayout>>>,
}

impl ErasureLayouts {
    pub fn binding(&self, id: VarId) -> ErasureLayout {
        self.bindings.get(&id).cloned().unwrap_or_default()
    }

    pub fn function(&self, id: FnRef) -> ErasureLayout {
        self.functions.get(&id).cloned().unwrap_or_default()
    }

    pub fn expression(&self, expr: &Expr, defs: &Definitions) -> ErasureLayout {
        let intrinsic = |ty: &Type| {
            if defs.is_erased_type(ty) {
                ErasureLayout::Logical
            } else {
                ErasureLayout::Default
            }
        };
        match expr {
            Expr::BoxNew { value, .. } => {
                ErasureLayout::Boxed(Box::new(self.expression(value, defs)))
            }
            Expr::BoxDeref { value, ty } => match self.expression(value, defs) {
                ErasureLayout::Boxed(inner) => *inner,
                _ => intrinsic(ty),
            },
            Expr::Shared { value, lifetime } => ErasureLayout::Shared {
                lifetime: lifetime.clone(),
                inner: Box::new(self.expression(value, defs)),
            },
            Expr::Deref(value) => match self.expression(value, defs) {
                ErasureLayout::Shared { inner, .. } => *inner,
                other => other,
            },
            Expr::Ghost(_)
            | Expr::Prop(_)
            | Expr::Proof(_)
            | Expr::Int(_)
            | Expr::IntArith { .. }
            | Expr::LogicalApply { .. } => ErasureLayout::Logical,
            Expr::Var { id, ty, .. } => self
                .bindings
                .get(id)
                .cloned()
                .map(|layout| match layout {
                    ErasureLayout::Borrowed { inner, .. } => *inner,
                    other => other,
                })
                .unwrap_or_else(|| intrinsic(ty)),
            Expr::Tuple { fields, .. } => ErasureLayout::Tuple(
                fields
                    .iter()
                    .map(|field| self.expression(field, defs))
                    .collect(),
            ),
            Expr::Struct { id, .. } => intrinsic(&Type::Struct(*id)),
            Expr::Variant { id, .. } => intrinsic(&Type::Enum(*id)),
            Expr::Field {
                target, index, ty, ..
            } => {
                if intrinsic(ty).is_logical() {
                    return ErasureLayout::Logical;
                }
                let target_layout = self.expression(target, defs);
                if matches!(
                    target_layout,
                    ErasureLayout::Tuple(_) | ErasureLayout::Logical
                ) {
                    return target_layout.field(*index);
                }
                expression_type(target)
                    .and_then(|ty| match ty.nominal() {
                        Type::Struct(id) => self
                            .structs
                            .get(id)
                            .and_then(|fields| fields.get(*index))
                            .cloned(),
                        _ => None,
                    })
                    .unwrap_or_default()
            }
            Expr::CallMath { id, ty, .. } => self
                .functions
                .get(&FnRef::Math(*id))
                .cloned()
                .unwrap_or_else(|| intrinsic(ty)),
            Expr::CallFn { id, ty, .. } => self
                .functions
                .get(&FnRef::Exec(*id))
                .cloned()
                .unwrap_or_else(|| intrinsic(ty)),
            Expr::Lend { value, .. } => self.expression(value, defs),
            Expr::Block(block) => block
                .tail
                .as_deref()
                .map(|tail| self.expression(tail, defs))
                .unwrap_or_default(),
            Expr::If {
                result,
                then_block,
                else_block,
                ty,
                ..
            } => self.bindings.get(result).cloned().unwrap_or_else(|| {
                [then_block, else_block]
                    .iter()
                    .filter_map(|block| block.tail.as_deref())
                    .map(|tail| self.expression(tail, defs))
                    .find(|layout| *layout != ErasureLayout::Default)
                    .unwrap_or_else(|| intrinsic(ty))
            }),
            Expr::Match {
                result, arms, ty, ..
            } => self.bindings.get(result).cloned().unwrap_or_else(|| {
                arms.iter()
                    .filter_map(|arm| arm.body.tail.as_deref())
                    .map(|tail| self.expression(tail, defs))
                    .find(|layout| *layout != ErasureLayout::Default)
                    .unwrap_or_else(|| intrinsic(ty))
            }),
            Expr::Loop { result, ty, .. }
            | Expr::Return { result, ty, .. }
            | Expr::Panic { result, ty, .. } => self
                .bindings
                .get(result)
                .cloned()
                .unwrap_or_else(|| intrinsic(ty)),
            Expr::Cast { to, .. } | Expr::Absurd { ty: to, .. } => intrinsic(to),
            _ => ErasureLayout::Default,
        }
    }
}

pub(super) fn expression_type(expr: &Expr) -> Option<Type> {
    match expr {
        Expr::BoxNew { value, .. } => expression_type(value).map(|ty| Type::Boxed(Box::new(ty))),
        Expr::BoxDeref { ty, .. } => Some(ty.clone()),
        Expr::Var { ty, .. }
        | Expr::Tuple { ty, .. }
        | Expr::Field { ty, .. }
        | Expr::CallMath { ty, .. }
        | Expr::LogicalApply { ty, .. }
        | Expr::CallFn { ty, .. }
        | Expr::If { ty, .. }
        | Expr::Match { ty, .. }
        | Expr::Loop { ty, .. }
        | Expr::Absurd { ty, .. }
        | Expr::Panic { ty, .. }
        | Expr::Return { ty, .. } => Some(ty.clone()),
        Expr::Struct { id, .. } => Some(Type::Struct(*id)),
        Expr::Variant { id, .. } => Some(Type::Enum(*id)),
        Expr::Lend { value, .. }
        | Expr::Ghost(value)
        | Expr::Shared { value, .. }
        | Expr::Deref(value) => expression_type(value),
        Expr::Block(block) => block.tail.as_deref().and_then(expression_type),
        _ => None,
    }
}

pub(super) fn collect_lifetimes(layout: &ErasureLayout, names: &mut Vec<String>) {
    match layout {
        ErasureLayout::Shared { lifetime, inner } | ErasureLayout::Borrowed { lifetime, inner } => {
            if let Some(l) = lifetime
                && !names.contains(l)
            {
                names.push(l.clone());
            }
            collect_lifetimes(inner, names)
        }
        ErasureLayout::Tuple(fields) => {
            for f in fields {
                collect_lifetimes(f, names)
            }
        }
        ErasureLayout::Boxed(inner) | ErasureLayout::Buffer { element: inner, .. } => {
            collect_lifetimes(inner, names)
        }
        ErasureLayout::NominalLifetimes(args) => {
            for l in args {
                if !names.contains(l) {
                    names.push(l.clone());
                }
            }
        }
        _ => {}
    }
}
