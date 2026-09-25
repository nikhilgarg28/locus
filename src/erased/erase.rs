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
//! - An erasable computation. Calls to checked logical definitions erase;
//!   ordinary calls remain even when they return only logical values or
//!   carry purity promises. Any runtime effects used to compute arguments
//!   of an erased call are kept in order before its marker. Branches keep
//!   their control flow when their arms contain such effects.
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
//! `mut` is printed on a binding only when the function assigns to it, or
//! lends it by `&mut`. A call with `&mut` arguments stays a call with
//! `&mut` arguments: the versions its write-backs make are versions of the
//! lent bindings, and the tuple the check IR returns is nowhere here; a
//! `&mut` parameter is a parameter, assigned in place.

use std::collections::{HashMap, HashSet};

use crate::exec::Program;
use crate::kernel::{MachineInt, Type, VarId};
use crate::typed::{
    Binder, Block, Carried, EnumItem, ErasureLayout, ErasureLayouts, Expr, FnItem, FnRef, Joined,
    Passing, Pattern, Place, Stmt, StructItem, each_expr, each_stmt, each_stmt_under,
};

use super::tree::{
    EArm, EBlock, EEnum, EExpr, EFn, EPattern, EPlace, EStmt, EStruct, EType, EVariant,
};

pub fn erase_type(ty: &Type) -> EType {
    match ty {
        Type::Instance(base, _) => erase_type(base),
        Type::Boxed(element) => EType::Boxed(Box::new(erase_type(element))),
        Type::Buffer(element) => EType::Buffer(Box::new(erase_type(element))),
        Type::Bool => EType::Bool,
        Type::U8 => EType::Int(crate::kernel::MachineInt::U8),
        Type::Machine(ty) => EType::Int(*ty),
        Type::Proof(_) => EType::Proved,
        Type::Prop | Type::Int => EType::Ghost,
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

/// Erasure with the checked declaration registry: a logical nominal type
/// has no runtime tag, even when its constructor carries no fields.
pub fn erase_type_in(program: &Program, ty: &Type) -> EType {
    if program.definitions().is_erased_type(ty) {
        return if matches!(ty, Type::Proof(_)) {
            EType::Proved
        } else {
            EType::Ghost
        };
    }
    match ty {
        Type::Boxed(element) => EType::Boxed(Box::new(erase_type_in(program, element))),
        Type::Tuple(fields) => EType::Tuple(
            fields
                .iter()
                .map(|field| erase_type_in(program, field))
                .collect(),
        ),
        Type::Fn(parameters, result) => EType::Fn(
            parameters
                .iter()
                .map(|parameter| erase_type_in(program, parameter))
                .collect(),
            Box::new(erase_type_in(program, result)),
        ),
        _ => erase_type(ty),
    }
}

pub(crate) fn type_with_layout(
    ty: &Type,
    layout: &ErasureLayout,
    natural: &impl Fn(&Type) -> EType,
) -> EType {
    if let Type::Instance(base, _) = ty {
        return type_with_layout(base, layout, natural);
    }
    match (layout, ty) {
        (
            ErasureLayout::Shared { lifetime, inner } | ErasureLayout::Borrowed { lifetime, inner },
            _,
        ) => EType::Ref(
            lifetime.clone(),
            Box::new(type_with_layout(ty, inner, natural)),
        ),
        (ErasureLayout::NominalLifetimes(args), Type::Struct(id)) => {
            EType::StructApplied(*id, args.clone())
        }
        (ErasureLayout::NominalLifetimes(args), Type::Enum(id)) => {
            EType::EnumApplied(*id, args.clone())
        }
        (ErasureLayout::Boxed(layout), Type::Boxed(element)) => {
            EType::Boxed(Box::new(type_with_layout(element, layout, natural)))
        }
        (
            ErasureLayout::Buffer {
                storage,
                element: layout,
            },
            Type::Buffer(element),
        ) => {
            let element = Box::new(type_with_layout(element, layout, natural));
            match storage {
                crate::exec::BufferStorage::Array(length) => EType::Array(element, *length),
                crate::exec::BufferStorage::Slice => EType::Slice(element),
                crate::exec::BufferStorage::Vector => EType::Buffer(element),
            }
        }
        (ErasureLayout::Logical, Type::Proof(_)) => EType::Proved,
        (ErasureLayout::Logical, _) => EType::Ghost,
        (ErasureLayout::Tuple(layouts), Type::Tuple(fields)) => EType::Tuple(
            fields
                .iter()
                .enumerate()
                .map(|(index, field)| {
                    type_with_layout(
                        field,
                        layouts.get(index).unwrap_or(&ErasureLayout::Default),
                        natural,
                    )
                })
                .collect(),
        ),
        _ => natural(ty),
    }
}

fn binder_type_with_layout(
    program: Option<&Program>,
    binder: &Binder,
    layouts: &ErasureLayouts,
) -> EType {
    let layout = if binder.ghost {
        ErasureLayout::Logical
    } else {
        layouts.binding(binder.id)
    };
    type_with_layout(&binder.ty, &layout, &|ty| {
        program.map_or_else(|| erase_type(ty), |program| erase_type_in(program, ty))
    })
}

/// Whether a binding has no runtime form: declared `Ghost<T>`, or of a
/// type whose erasure is the `Ghost` marker, `Prop` or `Int`. Evidence is
/// not among them: a proof-typed variable stays a variable of `Proved`.
fn vanishes(program: &Program, binder: &Binder, layouts: &ErasureLayouts) -> bool {
    binder_type_with_layout(Some(program), binder, layouts) == EType::Ghost
}

pub fn erase_struct(id: crate::kernel::StructId, item: &StructItem) -> EStruct {
    erase_struct_with_layout(id, item, &ErasureLayouts::default())
}

pub(crate) fn erase_struct_with_layout(
    id: crate::kernel::StructId,
    item: &StructItem,
    layouts: &ErasureLayouts,
) -> EStruct {
    EStruct {
        id,
        name: item.name.clone(),
        fields: item
            .fields
            .iter()
            .map(|field| {
                (
                    field.name.clone(),
                    binder_type_with_layout(None, field, layouts),
                )
            })
            .collect(),
        derives: item.derives.clone(),
    }
}

pub fn erase_enum(id: crate::kernel::EnumId, item: &EnumItem) -> EEnum {
    erase_enum_with_layout(id, item, &ErasureLayouts::default())
}

pub(crate) fn erase_enum_with_layout(
    id: crate::kernel::EnumId,
    item: &EnumItem,
    layouts: &ErasureLayouts,
) -> EEnum {
    EEnum {
        id,
        name: item.name.clone(),
        variants: item
            .variants
            .iter()
            .map(|variant| EVariant {
                name: variant.name.clone(),
                payload: variant
                    .payload
                    .iter()
                    .map(|binder| binder_type_with_layout(None, binder, layouts))
                    .collect(),
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
    erase_fn_with_layout(program, reference, item, &ErasureLayouts::default())
}

pub(crate) fn erase_fn_with_layout(
    program: &Program,
    reference: FnRef,
    item: &FnItem,
    layouts: &ErasureLayouts,
) -> Option<EFn> {
    if let FnRef::Math(id) = reference
        && !program.definitions().is_executable(id)
    {
        return None;
    }
    let mut eraser = Eraser {
        program,
        layouts,
        return_layout: layouts.function(reference),
        loop_layouts: Vec::new(),
        binding: HashMap::new(),
        assigned: assigned_bindings(&item.body),
        ghost: item
            .params
            .iter()
            .filter(|param| vanishes(program, param, layouts))
            .map(|param| param.id)
            .collect(),
    };
    // A `mut` parameter the body never assigns is printed without it.
    let passing = item
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| match item.passing_of(index) {
            Passing::MutValue if !eraser.assigned.contains(&param.id) => Passing::Value,
            passing => passing,
        })
        .collect();
    Some(EFn {
        reference,
        name: item.name.clone(),
        constant: false,
        params: item
            .params
            .iter()
            .map(|binder| {
                (
                    binder.id,
                    binder.name.clone(),
                    binder_type_with_layout(Some(program), binder, layouts),
                )
            })
            .collect(),
        passing,
        result: type_with_layout(&item.result, &layouts.function(reference), &|ty| {
            erase_type_in(program, ty)
        }),
        body: eraser.block_with_layout(&item.body, &layouts.function(reference)),
        owner: None,
        receiver: false,
    })
}

/// The bindings the body assigns to, whole or by a field, or lends by
/// `&mut`: the ones whose `let mut` needs its `mut`.
fn assigned_bindings(body: &Block) -> HashSet<VarId> {
    let mut assigned = HashSet::new();
    each_stmt(body, &mut |stmt| {
        if let Stmt::Assign { place, .. } = stmt {
            assigned.insert(place.binding);
        }
    });
    let mut on_expr = |expr: &Expr| {
        if let Expr::Lend {
            mutable: true,
            place,
            ..
        } = expr
        {
            assigned.insert(place.binding);
        }
    };
    for stmt in &body.stmts {
        match stmt {
            Stmt::Let { value, .. } | Stmt::Expr(value) | Stmt::Assign { value, .. } => {
                each_expr(value, &mut on_expr);
            }
        }
    }
    if let Some(tail) = body.tail.as_deref() {
        each_expr(tail, &mut on_expr);
    }
    assigned
}

/// The place of an assignment or a lend, its root at the binding.
fn place(place: &Place) -> EPlace {
    EPlace {
        id: place.binding,
        name: place.name.clone(),
        path: place
            .path
            .iter()
            .map(|step| (step.index, step.name.clone()))
            .collect(),
    }
}

struct Eraser<'p> {
    program: &'p Program,
    layouts: &'p ErasureLayouts,
    return_layout: ErasureLayout,
    loop_layouts: Vec<ErasureLayout>,
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

    /// Kernel functions are total and effect-free. Ordinary calls retain
    /// their execution independently of the erasure of their result.
    fn erasable(&self, callee: FnRef) -> bool {
        match callee {
            FnRef::Math(_) => true,
            FnRef::Exec(_) => false,
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
                Expr::BoxNew { .. }
                | Expr::Loop { .. }
                | Expr::While { .. }
                | Expr::For { .. }
                | Expr::Break(_)
                | Expr::Continue
                | Expr::Return { .. }
                | Expr::Operate { .. }
                | Expr::Panic { .. }
                | Expr::Assert { .. }
                | Expr::Absurd { .. } => true,
                // These forms contribute only the effects of their children,
                // visited exhaustively by each_expr. Native collections lower
                // to CallFn; primitive methods are total and allocation-free.
                Expr::BoxDeref { .. }
                | Expr::Shared { .. }
                | Expr::Deref(_)
                | Expr::LogicalApply { .. }
                | Expr::Var { .. }
                | Expr::Bool(_)
                | Expr::Literal(..)
                | Expr::Int(_)
                | Expr::Tuple { .. }
                | Expr::Struct { .. }
                | Expr::Variant { .. }
                | Expr::Field { .. }
                | Expr::Method { .. }
                | Expr::IntArith { .. }
                | Expr::Compare { .. }
                | Expr::Cast { .. }
                | Expr::CallMath { .. }
                | Expr::Lend { .. }
                | Expr::If { .. }
                | Expr::Match { .. }
                | Expr::Block(_)
                | Expr::Proof(_)
                | Expr::Prop(_)
                | Expr::Ghost(_) => false,
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
        let result = marker(self.program, ty);
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

    fn block_with_layout(&mut self, block: &Block, layout: &ErasureLayout) -> EBlock {
        EBlock {
            stmts: block
                .stmts
                .iter()
                .filter_map(|stmt| self.stmt(stmt))
                .collect(),
            tail: block
                .tail
                .as_deref()
                .map(|tail| Box::new(self.expr_with_layout(tail, layout))),
        }
    }

    fn expr_with_layout(&mut self, expr: &Expr, layout: &ErasureLayout) -> EExpr {
        match (expr, layout) {
            (_, ErasureLayout::Default) => self.expr(expr),
            (Expr::Ghost(_), _) => self.expr(expr),
            (Expr::Block(block), _) => EExpr::Block(self.block_with_layout(block, layout)),
            (
                Expr::If {
                    condition,
                    then_block,
                    else_block,
                    joined,
                    ..
                },
                _,
            ) if self.has_effects(expr) || !layout.is_logical() => {
                let result = EExpr::If {
                    condition: Box::new(self.expr(condition)),
                    then_block: self.block_with_layout(then_block, layout),
                    else_block: self.block_with_layout(else_block, layout),
                };
                self.joined(joined.as_ref());
                result
            }
            (Expr::Tuple { fields, .. }, ErasureLayout::Tuple(layouts)) => EExpr::Tuple(
                fields
                    .iter()
                    .enumerate()
                    .map(|(index, field)| {
                        self.expr_with_layout(
                            field,
                            layouts.get(index).unwrap_or(&ErasureLayout::Default),
                        )
                    })
                    .collect(),
            ),
            (_, ErasureLayout::Logical) => self.effects_then(
                std::slice::from_ref(expr),
                if expr.is_proof() {
                    EExpr::Proved
                } else {
                    EExpr::Ghost
                },
            ),
            _ => self.expr(expr),
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
                    place: self::place(place),
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
            Pattern::Bind { binder, .. } if vanishes(self.program, binder, self.layouts) => {
                self.ghost.insert(binder.id);
                EPattern::Wildcard
            }
            Pattern::Bind {
                binder, mutable, ..
            } => EPattern::Bind {
                id: binder.id,
                name: binder.name.clone(),
                ty: binder_type_with_layout(Some(self.program), binder, self.layouts),
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
            Expr::Prop(_) | Expr::Int(_) => EExpr::Ghost,
            Expr::IntArith { operands, .. } => self.effects_then(operands, EExpr::Ghost),
            Expr::LogicalApply { arguments, ty, .. } => self.effects_then(
                arguments,
                if matches!(ty, Type::Proof(_)) {
                    EExpr::Proved
                } else {
                    EExpr::Ghost
                },
            ),
            // The evidence and the learned facts are logical; the operation
            // stays the operator it is, at its type.
            Expr::Operate {
                op,
                ty,
                operands,
                fits,
                ..
            } => EExpr::Operate {
                op: *op,
                ty: *ty,
                operands: self.all(operands),
                proven_safe: fits.is_some(),
            },
            // The empty match: a marker when it stands for a ghost, a trap
            // when it stands for a value.
            Expr::Absurd { ty, .. } => marker(self.program, ty).unwrap_or(EExpr::Trap),
            // A `Ghost<T>` value is the marker. What stands inside it was
            // elaborated where nothing runs, so it does nothing; should it
            // still, that is kept, as for the arguments of a removed call.
            Expr::Ghost(inner) if inner.is_proof() => {
                self.effects_then(std::slice::from_ref(inner), EExpr::Proved)
            }
            Expr::Ghost(inner) => match &**inner {
                Expr::Compare { left, right, .. } => {
                    self.effects_then(&[(**left).clone(), (**right).clone()], EExpr::Ghost)
                }
                Expr::CallMath { arguments, id, .. }
                    if !self.program.definitions().is_executable(*id) =>
                {
                    self.effects_then(arguments, EExpr::Ghost)
                }
                Expr::If { .. } => self.expr_with_layout(inner, &ErasureLayout::Logical),
                _ => self.effects_then(std::slice::from_ref(inner), EExpr::Ghost),
            },
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
            Expr::Struct { id, fields, .. }
                if self
                    .program
                    .definitions()
                    .is_erased_type(&Type::Struct(*id)) =>
            {
                let values: Vec<Expr> = fields.iter().map(|(_, value)| value.clone()).collect();
                self.effects_then(&values, EExpr::Ghost)
            }
            Expr::Variant { id, payload, .. }
                if self.program.definitions().is_erased_type(&Type::Enum(*id)) =>
            {
                self.effects_then(payload, EExpr::Ghost)
            }
            Expr::Struct {
                id, name, fields, ..
            } => EExpr::Struct {
                id: *id,
                name: name.clone(),
                fields: fields
                    .iter()
                    .enumerate()
                    .map(|(index, (field, value))| {
                        (
                            field.clone(),
                            self.expr_with_layout(
                                value,
                                &self
                                    .layouts
                                    .structs
                                    .get(id)
                                    .and_then(|fields| fields.get(index))
                                    .cloned()
                                    .unwrap_or_default(),
                            ),
                        )
                    })
                    .collect(),
            },
            Expr::Variant {
                id,
                enum_name,
                index,
                variant_name,
                payload,
                ..
            } => EExpr::Variant {
                id: *id,
                enum_name: enum_name.clone(),
                index: *index,
                variant_name: variant_name.clone(),
                payload: payload
                    .iter()
                    .enumerate()
                    .map(|(payload_index, value)| {
                        self.expr_with_layout(
                            value,
                            &self
                                .layouts
                                .enums
                                .get(id)
                                .and_then(|variants| variants.get(*index))
                                .and_then(|fields| fields.get(payload_index))
                                .cloned()
                                .unwrap_or_default(),
                        )
                    })
                    .collect(),
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
                _ => self.effects_then(std::slice::from_ref(expr), EExpr::Ghost),
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
                lends,
                ..
            } => {
                // A call with `&mut` arguments writes what its caller can
                // see: it is never erased, whatever the callee promises and
                // whatever its result. The versions the write-backs make
                // are versions of the lent bindings, which the call assigns
                // in place.
                let erased = if lends.is_empty() {
                    self.call(FnRef::Exec(*id), name, arguments, ty)
                } else {
                    EExpr::Call {
                        callee: FnRef::Exec(*id),
                        name: name.clone(),
                        arguments: self.all(arguments),
                    }
                };
                for lend in lends {
                    if let Some(Expr::Lend { place, .. }) = arguments.get(lend.argument) {
                        self.binding.insert(lend.version.id, place.binding);
                    }
                }
                erased
            }
            Expr::Shared { value, lifetime } => EExpr::Shared {
                value: Box::new(self.expr(value)),
                lifetime: lifetime.clone(),
            },
            Expr::BoxNew { value, .. } => EExpr::BoxNew(Box::new(self.expr(value))),
            Expr::BoxDeref { value, .. } => EExpr::BoxDeref(Box::new(self.expr(value))),
            Expr::Deref(value) => EExpr::Deref(Box::new(self.expr(value))),
            Expr::Lend { mutable, place, .. } => EExpr::Lend {
                mutable: *mutable,
                place: self::place(place),
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
                let scrutinee = Box::new(self.expr(scrutinee));
                let mut erased_arms = Vec::new();
                for arm in arms {
                    // A payload with no runtime form is bound to the marker
                    // the variant holds; a mention of it is the marker.
                    for binder in arm
                        .payload
                        .iter()
                        .filter(|binder| vanishes(self.program, binder, self.layouts))
                    {
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
                result,
                body,
                ..
            } => {
                self.carried(state, carried);
                let layout = self.layouts.binding(*result);
                self.loop_layouts.push(layout.clone());
                let body = self.block(body);
                self.loop_layouts.pop();
                EExpr::Loop {
                    result: type_with_layout(ty, &layout, &|ty| erase_type_in(self.program, ty)),
                    body,
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
                self.loop_layouts.push(ErasureLayout::Default);
                let condition = Box::new(self.expr(condition));
                let body = self.block(body);
                self.loop_layouts.pop();
                EExpr::While { condition, body }
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
                let lo = Box::new(self.expr(lo));
                let hi = Box::new(self.expr(hi));
                self.loop_layouts.push(ErasureLayout::Default);
                let body = self.block(body);
                self.loop_layouts.pop();
                EExpr::For {
                    index: (index.id, index.name.clone(), ty),
                    lo,
                    hi,
                    inclusive: *inclusive,
                    body,
                }
            }
            Expr::Break(value) => {
                let layout = self.loop_layouts.last().cloned().unwrap_or_default();
                EExpr::Break(
                    value
                        .as_deref()
                        .map(|value| Box::new(self.expr_with_layout(value, &layout))),
                )
            }
            Expr::Continue => EExpr::Continue,
            // `return` with no value returns `()`; the printer writes it
            // bare again.
            Expr::Return { value, .. } => EExpr::Return(Box::new(match value {
                Some(value) => self.expr_with_layout(value, &self.return_layout.clone()),
                None => EExpr::Tuple(Vec::new()),
            })),
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
fn marker(program: &Program, ty: &Type) -> Option<EExpr> {
    match erase_type_in(program, ty) {
        EType::Proved => Some(EExpr::Proved),
        EType::Ghost => Some(EExpr::Ghost),
        _ => None,
    }
}
