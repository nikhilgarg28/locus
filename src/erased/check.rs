//! A simple type checker for the erased tree. It guards `erase`: whatever
//! `erase` emits must be well typed here, with no propositions and no
//! dependency. A dangling reference to something that was not emitted, a
//! marker where data belongs, or a mistake in a loop's state fails here.
//!
//! It is a second judge of erasure's rule for values with no runtime form:
//! a `let` may not bind a name to a value of type `Ghost`, the erasure of
//! a proposition, an `Int`, or a `Ghost<T>`, since `erase` leaves such a
//! binding out and replaces every mention of it by the marker. A parameter
//! or a field of that type is a position and is allowed; evidence, of type
//! `Proved`, is bound as any value is.

use std::collections::HashMap;
use std::fmt;

use crate::kernel::MachineInt;
use crate::kernel::{Prim, VarId};
use crate::typed::{CompareOp, FnRef};

use super::tree::{EBlock, EExpr, EPattern, EPlace, EStmt, EType, Module};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeError(pub String);

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TypeError {}

fn fail<T>(message: impl Into<String>) -> Result<T, TypeError> {
    Err(TypeError(message.into()))
}

/// What `break` refers to: the type of the value it carries in a `loop`,
/// or `None` in a `while` or a `for`, where it carries nothing.
#[derive(Clone)]
struct Target {
    result: Option<EType>,
}

struct Checker<'m> {
    module: &'m Module,
    signatures: HashMap<FnRef, (Vec<EType>, EType)>,
    /// The result type of the function being checked, for `return`.
    result: EType,
    /// Each name in scope with its type and whether it may be assigned.
    env: Vec<(VarId, EType, bool)>,
    targets: Vec<Target>,
}

/// The type of an expression, or `None` when it never yields a value
/// because it transfers control, traps, or panics. An expression that never
/// yields is accepted wherever a value of any type is expected, and an
/// expression that needs the value of one that never yields never yields
/// either: `f(panic!("..."))` does not call `f`. That is all it excuses.
/// Whatever stands around or after it is checked as it would be anyway: the
/// other arguments, the rest of the block, every arm.
type Yield = Option<EType>;

/// The type of a subexpression whose value is needed; when there is none,
/// the expression being checked never yields.
macro_rules! needed {
    ($found:expr) => {
        match $found {
            Some(ty) => ty,
            None => return Ok(None),
        }
    };
}

pub fn check_module(module: &Module) -> Result<(), TypeError> {
    let signatures = module
        .fns
        .iter()
        .map(|function| {
            let params = function
                .params
                .iter()
                .enumerate()
                .map(|(i, (_, _, ty))| parameter_type(ty, function.passing_of(i)))
                .collect();
            (function.reference, (params, function.result.clone()))
        })
        .collect();
    let mut checker = Checker {
        module,
        signatures,
        result: EType::unit(),
        env: Vec::new(),
        targets: Vec::new(),
    };
    for function in &module.fns {
        // A `mut` or `&mut` parameter may be assigned.
        checker.env = function
            .params
            .iter()
            .enumerate()
            .map(|(index, (id, _, ty))| {
                (
                    *id,
                    parameter_type(ty, function.passing_of(index)),
                    function.passing_of(index).is_mutable(),
                )
            })
            .collect();
        checker.targets.clear();
        checker.result = function.result.clone();
        let found = checker.block(&function.body)?;
        expect(
            &found,
            &function.result,
            &format!("the body of {}", function.name),
        )?;
    }
    Ok(())
}

fn expect(found: &Yield, expected: &EType, what: &str) -> Result<(), TypeError> {
    match found {
        None => Ok(()),
        Some(found) if same_shape(found, expected) => Ok(()),
        Some(found) => fail(format!("{what} has type {found:?}, expected {expected:?}")),
    }
}

/// The type both branches yield, when they yield at all.
fn join(left: Yield, right: Yield, what: &str) -> Result<Yield, TypeError> {
    match (left, right) {
        (None, other) | (other, None) => Ok(other),
        (Some(left), Some(right)) if same_shape(&left, &right) => Ok(Some(left)),
        (Some(left), Some(right)) => fail(format!("{what} disagree: {left:?} and {right:?}")),
    }
}

impl Checker<'_> {
    fn block(&mut self, block: &EBlock) -> Result<Yield, TypeError> {
        let scope = self.env.len();
        let result = self.block_in_scope(block);
        self.env.truncate(scope);
        result
    }

    /// Every statement and the tail are checked, whatever came before them.
    /// A block that contains a statement that never yields never yields
    /// either, since its end is not reached.
    fn block_in_scope(&mut self, block: &EBlock) -> Result<Yield, TypeError> {
        let mut reaches_its_end = true;
        for stmt in &block.stmts {
            let found = match stmt {
                EStmt::Let { pattern, value } => {
                    let found = self.expr(value)?;
                    self.bind(pattern, found.as_ref())?;
                    found
                }
                EStmt::Assign { place, value } => {
                    let found = self.expr(value)?;
                    let expected = self.place(place, true)?;
                    expect(
                        &found,
                        &expected,
                        &format!("the value assigned to {}", place.name),
                    )?;
                    found.map(|_| EType::unit())
                }
                EStmt::Expr(expr) => self.expr(expr)?,
            };
            reaches_its_end &= found.is_some();
        }
        let tail = match &block.tail {
            Some(tail) => self.expr(tail)?,
            None => Some(EType::unit()),
        };
        Ok(tail.filter(|_| reaches_its_end))
    }

    /// Binds the names of a pattern at the types they carry. `found` is the
    /// type of the value, which those types must agree with; it is `None`
    /// when the value never yields, and the names are still bound, so that
    /// what follows is checked.
    fn bind(&mut self, pattern: &EPattern, found: Option<&EType>) -> Result<(), TypeError> {
        match pattern {
            EPattern::Wildcard => Ok(()),
            EPattern::Bind {
                id,
                name,
                ty,
                mutable,
            } => {
                if *ty == EType::Ghost {
                    return fail(format!(
                        "{name} is bound to a value with no runtime form, which erasure leaves out"
                    ));
                }
                if found.is_some_and(|found| !same_shape(found, ty)) {
                    return fail(format!("{name} is bound as {ty:?} to {found:?}"));
                }
                self.env.push((*id, ty.clone(), *mutable));
                Ok(())
            }
            EPattern::Tuple(patterns) => match found {
                None => patterns
                    .iter()
                    .try_for_each(|pattern| self.bind(pattern, None)),
                Some(EType::Tuple(fields)) if fields.len() == patterns.len() => patterns
                    .iter()
                    .zip(fields)
                    .try_for_each(|(pattern, field)| self.bind(pattern, Some(field))),
                Some(other) => fail(format!("a tuple pattern against {other:?}")),
            },
        }
    }

    /// The type of a place: the binding must be in scope and, when it is
    /// written to, assigned to or lent by `&mut`, assignable; the path must
    /// step through products that have those fields.
    fn place(&mut self, place: &EPlace, written: bool) -> Result<EType, TypeError> {
        let Some((_, ty, mutable)) = self.env.iter().rev().find(|(var, _, _)| *var == place.id)
        else {
            return fail(format!("{} is not in scope", place.name));
        };
        if written && !mutable {
            return fail(format!("{} is assigned but not declared mut", place.name));
        }
        let mut ty = ty.clone();
        for (index, name) in &place.path {
            let fields = match &ty {
                EType::Tuple(fields) => fields.clone(),
                EType::Struct(id) | EType::StructApplied(id, _) => {
                    match self.module.structs.iter().find(|d| d.id == *id) {
                        Some(decl) => decl.fields.iter().map(|(_, ty)| ty.clone()).collect(),
                        None => return fail("assignment into a struct that was not emitted"),
                    }
                }
                other => return fail(format!("assignment into a field of {other:?}")),
            };
            ty = match fields.get(*index) {
                Some(field) => field.clone(),
                None => {
                    let shown = name.clone().unwrap_or_else(|| index.to_string());
                    return fail(format!("{} has no field {shown}", place.name));
                }
            };
        }
        Ok(ty)
    }

    /// Subexpressions whose values are all needed. Every one is checked, and
    /// there are types only when every one yields.
    fn values<'e>(
        &mut self,
        exprs: impl IntoIterator<Item = &'e EExpr>,
    ) -> Result<Option<Vec<EType>>, TypeError> {
        let mut types = Some(Vec::new());
        for expr in exprs {
            match (self.expr(expr)?, &mut types) {
                (Some(ty), Some(types)) => types.push(ty),
                _ => types = None,
            }
        }
        Ok(types)
    }

    /// Values for positions of known types: one for each position, and each
    /// that yields has the type of its position. `None` when one never
    /// yields.
    fn arguments<'e>(
        &mut self,
        arguments: impl IntoIterator<Item = &'e EExpr>,
        expected: &[EType],
        what: &str,
        passing: Option<&[crate::typed::Passing]>,
    ) -> Result<Option<()>, TypeError> {
        let mut yields = Some(());
        let mut count = 0;
        for (position, argument) in arguments.into_iter().enumerate() {
            count += 1;
            match (self.expr(argument)?, expected.get(position)) {
                (Some(found), Some(expected))
                    if !same_shape(&found, expected)
                        && !slice_coercion(
                            &found,
                            expected,
                            argument,
                            passing.and_then(|p| p.get(position)).copied(),
                        ) =>
                {
                    return fail(format!(
                        "{what}: found {found:?} at {position}, expected {expected:?}"
                    ));
                }
                (None, _) => yields = None,
                _ => {}
            }
        }
        if count != expected.len() {
            return fail(format!(
                "{what}: found {count} value(s), expected {}",
                expected.len()
            ));
        }
        Ok(yields)
    }

    fn expr(&mut self, expr: &EExpr) -> Result<Yield, TypeError> {
        Ok(Some(match expr {
            EExpr::Shared { value, lifetime } => {
                EType::Ref(lifetime.clone(), Box::new(needed!(self.expr(value)?)))
            }
            EExpr::BoxNew(value) => EType::Boxed(Box::new(needed!(self.expr(value)?))),
            EExpr::BoxDeref(value) => match needed!(self.expr(value)?) {
                EType::Boxed(inner) => *inner,
                other => return fail(format!("cannot unbox {other:?}")),
            },
            EExpr::Deref(value) => match needed!(self.expr(value)?) {
                EType::Ref(_, inner) => *inner,
                other => return fail(format!("cannot dereference {other:?}")),
            },
            EExpr::Buffer {
                op,
                storage,
                element,
                arguments,
            } => {
                let buffer = match storage {
                    crate::exec::BufferStorage::Array(n) => {
                        EType::Array(Box::new(element.clone()), *n)
                    }
                    crate::exec::BufferStorage::Slice => EType::Slice(Box::new(element.clone())),
                    crate::exec::BufferStorage::Vector => EType::Buffer(Box::new(element.clone())),
                };
                let expected = match op {
                    crate::kernel::BufferOp::Literal => vec![element.clone(); arguments.len()],
                    crate::kernel::BufferOp::Length => vec![buffer.clone()],
                    crate::kernel::BufferOp::Get => {
                        vec![buffer.clone(), EType::Int(MachineInt::U64)]
                    }
                    crate::kernel::BufferOp::Set => {
                        vec![buffer.clone(), EType::Int(MachineInt::U64), element.clone()]
                    }
                    crate::kernel::BufferOp::Push => vec![buffer.clone(), element.clone()],
                };
                let actual = needed!(self.values(arguments)?);
                if actual != expected {
                    return fail("native buffer argument types differ");
                }
                if matches!(
                    op,
                    crate::kernel::BufferOp::Set | crate::kernel::BufferOp::Push
                ) && !matches!(arguments.first(), Some(EExpr::Lend { mutable: true, .. }))
                {
                    return fail("buffer mutation needs a mutable receiver");
                }
                match op {
                    crate::kernel::BufferOp::Literal => {
                        if matches!(storage, crate::exec::BufferStorage::Slice) {
                            return fail("cannot construct an unsized slice");
                        }
                        if let crate::exec::BufferStorage::Array(n) = storage
                            && *n != arguments.len()
                        {
                            return fail("array literal length differs");
                        }
                        buffer
                    }
                    crate::kernel::BufferOp::Length => EType::Int(MachineInt::U64),
                    crate::kernel::BufferOp::Get => element.clone(),
                    crate::kernel::BufferOp::Set | crate::kernel::BufferOp::Push => EType::unit(),
                }
            }
            EExpr::Var { id, name } => match self.env.iter().rev().find(|(var, _, _)| var == id) {
                Some((_, ty, _)) => ty.clone(),
                None => return fail(format!("{name} is not in scope")),
            },
            EExpr::Bool(_) => EType::Bool,
            EExpr::Literal(ty, value) => {
                if !ty.contains(&crate::kernel::Integer::from(*value)) {
                    return fail(format!("{value} is not a value of {}", ty.name()));
                }
                EType::Int(*ty)
            }
            EExpr::Proved => EType::Proved,
            EExpr::Ghost => EType::Ghost,
            EExpr::Trap | EExpr::Panic { .. } => return Ok(None),
            EExpr::Assert { condition, .. } => {
                let condition = self.expr(condition)?;
                if condition.as_ref().is_some_and(|ty| *ty != EType::Bool) {
                    return fail("an assertion of something that is not a bool");
                }
                return Ok(condition.map(|_| EType::unit()));
            }
            EExpr::Tuple(fields) => EType::Tuple(needed!(self.values(fields)?)),
            EExpr::Struct { id, name, fields } => {
                let Some(decl) = self.module.structs.iter().find(|decl| decl.id == *id) else {
                    return fail(format!("struct {name} was not emitted"));
                };
                let expected: Vec<EType> = decl.fields.iter().map(|(_, ty)| ty.clone()).collect();
                let values = fields.iter().map(|(_, value)| value);
                needed!(self.arguments(
                    values,
                    &expected,
                    &format!("the fields of {name}"),
                    None
                )?);
                EType::Struct(*id)
            }
            EExpr::Variant {
                id,
                enum_name,
                index,
                payload,
                ..
            } => {
                let Some(decl) = self.module.enums.iter().find(|decl| decl.id == *id) else {
                    return fail(format!("enum {enum_name} was not emitted"));
                };
                let Some(variant) = decl.variants.get(*index) else {
                    return fail(format!("{enum_name} has no variant {index}"));
                };
                let expected = variant.payload.clone();
                needed!(self.arguments(
                    payload,
                    &expected,
                    &format!("the payload of {}", variant.name),
                    None,
                )?);
                EType::Enum(*id)
            }
            EExpr::Field { target, index, .. } => {
                let mut target_ty = needed!(self.expr(target)?);
                while let EType::Ref(_, inner) = target_ty {
                    target_ty = *inner;
                }
                let fields = match target_ty {
                    EType::Tuple(fields) => fields,
                    EType::Struct(id) | EType::StructApplied(id, _) => {
                        match self.module.structs.iter().find(|d| d.id == id) {
                            Some(decl) => decl.fields.iter().map(|(_, ty)| ty.clone()).collect(),
                            None => return fail("projection from a struct that was not emitted"),
                        }
                    }
                    other => return fail(format!("projection from {other:?}")),
                };
                match fields.get(*index) {
                    Some(ty) => ty.clone(),
                    None => return fail(format!("no field {index}")),
                }
            }
            EExpr::Method {
                prim,
                receiver,
                arguments,
            } => {
                // A row of the table is the one primitive with a runtime
                // form that is called as a method.
                let Prim::Op(op, ty) = prim else {
                    return fail(format!("{} has no runtime form", prim.name()));
                };
                if op.row(*ty).is_none() {
                    return fail(format!("{} has no row at {}", op.name(), ty.name()));
                }
                let operands = std::iter::once(&**receiver).chain(arguments);
                let operands = needed!(self.values(operands)?);
                if operands.len() != op.arity()
                    || operands.iter().any(|found| *found != EType::Int(*ty))
                {
                    return fail(format!("{prim} applied to {operands:?}"));
                }
                EType::Int(*ty)
            }
            EExpr::Compare { op, left, right } => {
                let operands = needed!(self.values([&**left, &**right])?);
                let ordered =
                    matches!(operands.as_slice(), [EType::Int(a), EType::Int(b)] if a == b);
                let equated = matches!(op, CompareOp::Eq | CompareOp::Ne)
                    && operands == [EType::Bool, EType::Bool];
                if !ordered && !equated {
                    return fail(format!("a comparison of {operands:?}"));
                }
                EType::Bool
            }
            EExpr::Cast { expr, to } => {
                let found = needed!(self.expr(expr)?);
                if !matches!(found, EType::Int(_)) {
                    return fail(format!("a cast of {found:?}"));
                }
                EType::Int(*to)
            }
            EExpr::Operate {
                op, ty, operands, ..
            } => {
                if op.row(*ty).is_none() {
                    return fail(format!("{} has no row at {}", op.name(), ty.name()));
                }
                let found = needed!(self.values(operands)?);
                if found.len() != op.arity() || found.iter().any(|f| *f != EType::Int(*ty)) {
                    return fail(format!("{}[{}] applied to {found:?}", op.name(), ty.name()));
                }
                EType::Int(*ty)
            }
            EExpr::NativeCall {
                arguments, result, ..
            } => {
                let types = needed!(self.values(arguments)?);
                fn physical(ty: &EType) -> bool {
                    matches!(ty, EType::Bool | EType::Int(_))
                        || matches!(ty,EType::Tuple(fields) if fields.iter().all(physical))
                }
                if !physical(result) || !types.iter().all(physical) {
                    return fail(
                        "native call cannot inspect or create erased/invariant-bearing values",
                    );
                }
                result.clone()
            }
            EExpr::Call {
                callee,
                name,
                arguments,
            } => {
                let Some((params, result)) = self.signatures.get(callee).cloned() else {
                    return fail(format!("{name} was not emitted"));
                };
                let passing = self
                    .module
                    .fns
                    .iter()
                    .find(|f| f.reference == *callee)
                    .map(|f| f.passing.clone())
                    .unwrap_or_default();
                needed!(self.arguments(
                    arguments,
                    &params,
                    &format!("the arguments of {name}"),
                    Some(&passing)
                )?);
                result
            }
            // A lent place has the type of the place; a `&mut` one is
            // written to.
            EExpr::Lend { mutable, place } => self.place(place, *mutable)?,
            EExpr::If {
                condition,
                then_block,
                else_block,
            } => {
                let condition = self.expr(condition)?;
                if condition.as_ref().is_some_and(|ty| *ty != EType::Bool) {
                    return fail("a condition that is not a bool");
                }
                let then_type = self.block(then_block)?;
                let else_type = self.block(else_block)?;
                let joined = join(then_type, else_type, "the branches of an if")?;
                return Ok(condition.and(joined));
            }
            EExpr::Match {
                scrutinee,
                enum_name,
                arms,
            } => {
                // A scrutinee that never yields has no type to find the enum
                // by, and no arm runs. The arms are checked all the same,
                // against the enum the match names.
                let scrutinee = self.expr(scrutinee)?;
                let enums = &self.module.enums;
                let decl = match &scrutinee {
                    Some(EType::Enum(id) | EType::EnumApplied(id, _)) => {
                        enums.iter().find(|decl| decl.id == *id)
                    }
                    Some(_) => return fail("a match on something that is not an enum"),
                    None => enums.iter().find(|decl| decl.name == *enum_name),
                };
                let Some(decl) = decl else {
                    return fail("a match on an enum that was not emitted");
                };
                if decl.variants.len() != arms.len() {
                    return fail("a match without one arm per variant");
                }
                let variants = decl.variants.clone();
                let mut result = None;
                for (arm, variant) in arms.iter().zip(&variants) {
                    if arm.payload.len() != variant.payload.len() {
                        return fail(format!("arm {} binds the wrong payload", arm.variant_name));
                    }
                    let scope = self.env.len();
                    for ((id, _), ty) in arm.payload.iter().zip(&variant.payload) {
                        self.env.push((*id, ty.clone(), false));
                    }
                    let arm_type = self.block(&arm.body);
                    self.env.truncate(scope);
                    result = join(result, arm_type?, "the arms of a match")?;
                }
                return Ok(scrutinee.and(result));
            }
            EExpr::Block(block) => return self.block(block),
            EExpr::Loop { result, body } => {
                self.iterate(Some(result.clone()), body, None)?;
                result.clone()
            }
            EExpr::While { condition, body } => {
                let condition = self.expr(condition)?;
                if condition.as_ref().is_some_and(|ty| *ty != EType::Bool) {
                    return fail("a while condition that is not a bool");
                }
                self.iterate(None, body, None)?;
                needed!(condition);
                EType::unit()
            }
            EExpr::For {
                index,
                lo,
                hi,
                body,
                ..
            } => {
                let bounds = self.values([&**lo, &**hi])?;
                match bounds.as_deref() {
                    Some([EType::Int(lo), EType::Int(hi)]) if lo == hi && *lo == index.2 => {}
                    Some(_) => {
                        return fail(
                            "for bounds that are not two machine integers of the index's type",
                        );
                    }
                    // The bounds never yield, and the body is still checked.
                    None => {}
                }
                self.iterate(None, body, Some((index.0, index.2)))?;
                needed!(bounds);
                EType::unit()
            }
            EExpr::Break(value) => {
                let Some(target) = self.targets.last().cloned() else {
                    return fail("break outside a loop");
                };
                match (target.result, value) {
                    (None, None) => {}
                    (None, Some(_)) => return fail("break with a value inside a while or a for"),
                    (Some(result), value) => {
                        let found = match value {
                            Some(value) => needed!(self.expr(value)?),
                            None => EType::unit(),
                        };
                        if found != result {
                            return fail(format!(
                                "break with {found:?}, the loop yields {result:?}"
                            ));
                        }
                    }
                }
                return Ok(None);
            }
            EExpr::Continue => {
                if self.targets.is_empty() {
                    return fail("continue outside a loop");
                }
                return Ok(None);
            }
            EExpr::Return(value) => {
                let found = self.expr(value)?;
                let result = self.result.clone();
                expect(&found, &result, "a returned value")?;
                return Ok(None);
            }
        }))
    }

    /// Checks the body of a loop, a while, or a for: it sees the index if
    /// there is one, and where it reaches its end it yields `()`, which is
    /// the next pass.
    fn iterate(
        &mut self,
        result: Option<EType>,
        body: &EBlock,
        index: Option<(VarId, crate::kernel::MachineInt)>,
    ) -> Result<(), TypeError> {
        let scope = self.env.len();
        if let Some((index, ty)) = index {
            self.env.push((index, EType::Int(ty), false));
        }
        self.targets.push(Target { result });
        let yielded = self.block(body);
        self.targets.pop();
        self.env.truncate(scope);
        expect(&yielded?, &EType::unit(), "the body of a loop")
    }
}

fn slice_coercion(
    found: &EType,
    expected: &EType,
    argument: &EExpr,
    passing: Option<crate::typed::Passing>,
) -> bool {
    let Some(passing) = passing else {
        return false;
    };
    let EExpr::Lend { mutable, .. } = argument else {
        return false;
    };
    if !passing.is_reference() || (*mutable != (passing == crate::typed::Passing::RefMut)) {
        return false;
    }
    if let EType::Ref(_, inner) = found
        && passing == crate::typed::Passing::Ref
        && (same_shape(inner, expected)
            || matches!((&**inner,expected),(EType::Array(a,_)|EType::Buffer(a),EType::Slice(b)) if same_shape(a,b)))
    {
        return true;
    }
    match (found, expected) {
        (EType::Array(a, _), EType::Slice(b)) | (EType::Buffer(a), EType::Slice(b)) => a == b,
        _ => false,
    }
}

// Lifetimes are checked by the typed permission checker, independently from
// runtime shape. This checker still distinguishes a reference from its value.
fn parameter_type(ty: &EType, passing: crate::typed::Passing) -> EType {
    match ty {
        EType::Ref(_, inner) if passing.is_reference() => (**inner).clone(),
        _ => ty.clone(),
    }
}
fn same_shape(left: &EType, right: &EType) -> bool {
    match (left, right) {
        (EType::Ref(_, a), EType::Ref(_, b)) => same_shape(a, b),
        (EType::Tuple(a), EType::Tuple(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same_shape(a, b))
        }
        (
            EType::Struct(a) | EType::StructApplied(a, _),
            EType::Struct(b) | EType::StructApplied(b, _),
        ) => a == b,
        (EType::Enum(a) | EType::EnumApplied(a, _), EType::Enum(b) | EType::EnumApplied(b, _)) => {
            a == b
        }
        _ => left == right,
    }
}
