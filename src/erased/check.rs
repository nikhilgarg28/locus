//! A simple type checker for the erased tree. It guards `erase`: whatever
//! `erase` emits must be well typed here, with no propositions and no
//! dependency. A dangling reference to something that was not emitted, a
//! marker where data belongs, or a mistake in a loop's state fails here.

use std::collections::HashMap;
use std::fmt;

use crate::kernel::{Prim, VarId};
use crate::typed::FnRef;

use super::tree::{EBlock, EExpr, EPattern, EStmt, EType, Module};

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

/// What `break` and `continue` refer to.
#[derive(Clone)]
struct Target {
    state: Vec<EType>,
    /// `None` for a `for`, which has no `break`.
    result: Option<EType>,
}

struct Checker<'m> {
    module: &'m Module,
    signatures: HashMap<FnRef, (Vec<EType>, EType)>,
    env: Vec<(VarId, EType)>,
    targets: Vec<Target>,
}

/// The type of an expression, or `None` when it never yields a value
/// because it transfers control or traps.
type Yield = Option<EType>;

pub fn check_module(module: &Module) -> Result<(), TypeError> {
    let signatures = module
        .fns
        .iter()
        .map(|function| {
            let params = function
                .params
                .iter()
                .map(|(_, _, ty)| ty.clone())
                .collect();
            (function.reference, (params, function.result.clone()))
        })
        .collect();
    let mut checker = Checker {
        module,
        signatures,
        env: Vec::new(),
        targets: Vec::new(),
    };
    for function in &module.fns {
        checker.env = function
            .params
            .iter()
            .map(|(id, _, ty)| (*id, ty.clone()))
            .collect();
        checker.targets.clear();
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
        Some(found) if found == expected => Ok(()),
        Some(found) => fail(format!("{what} has type {found:?}, expected {expected:?}")),
    }
}

/// The type both branches yield, when they yield at all.
fn join(left: Yield, right: Yield, what: &str) -> Result<Yield, TypeError> {
    match (left, right) {
        (None, other) | (other, None) => Ok(other),
        (Some(left), Some(right)) if left == right => Ok(Some(left)),
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

    fn block_in_scope(&mut self, block: &EBlock) -> Result<Yield, TypeError> {
        for stmt in &block.stmts {
            match stmt {
                EStmt::Let { pattern, value } => {
                    if let Some(ty) = self.expr(value)? {
                        self.bind(pattern, &ty)?;
                    }
                }
                EStmt::Expr(expr) => {
                    self.expr(expr)?;
                }
            }
        }
        match &block.tail {
            Some(tail) => self.expr(tail),
            None => Ok(Some(EType::unit())),
        }
    }

    fn bind(&mut self, pattern: &EPattern, ty: &EType) -> Result<(), TypeError> {
        match pattern {
            EPattern::Wildcard => Ok(()),
            EPattern::Bind { id, .. } => {
                self.env.push((*id, ty.clone()));
                Ok(())
            }
            EPattern::Tuple(patterns) => match ty {
                EType::Tuple(fields) if fields.len() == patterns.len() => patterns
                    .iter()
                    .zip(fields)
                    .try_for_each(|(pattern, field)| self.bind(pattern, field)),
                other => fail(format!("a tuple pattern against {other:?}")),
            },
        }
    }

    /// A subexpression whose value is needed.
    fn value(&mut self, expr: &EExpr, what: &str) -> Result<EType, TypeError> {
        match self.expr(expr)? {
            Some(ty) => Ok(ty),
            None => fail(format!("{what} never yields a value")),
        }
    }

    fn values(&mut self, exprs: &[EExpr], what: &str) -> Result<Vec<EType>, TypeError> {
        exprs.iter().map(|expr| self.value(expr, what)).collect()
    }

    fn arguments(
        &mut self,
        arguments: &[EExpr],
        expected: &[EType],
        what: &str,
    ) -> Result<(), TypeError> {
        let found = self.values(arguments, what)?;
        if found == expected {
            Ok(())
        } else {
            fail(format!("{what}: found {found:?}, expected {expected:?}"))
        }
    }

    fn state(&mut self, state: &[(VarId, String, EType, EExpr)]) -> Result<Vec<EType>, TypeError> {
        let mut types = Vec::new();
        for (_, name, ty, init) in state {
            let found = self.value(init, "an initial state value")?;
            if &found != ty {
                return fail(format!("state {name} starts as {found:?}, declared {ty:?}"));
            }
            types.push(ty.clone());
        }
        Ok(types)
    }

    fn expr(&mut self, expr: &EExpr) -> Result<Yield, TypeError> {
        Ok(Some(match expr {
            EExpr::Var { id, name } => match self.env.iter().rev().find(|(var, _)| var == id) {
                Some((_, ty)) => ty.clone(),
                None => return fail(format!("{name} is not in scope")),
            },
            EExpr::Bool(_) => EType::Bool,
            EExpr::U8(_) => EType::U8,
            EExpr::Proved => EType::Proved,
            EExpr::Ghost => EType::Ghost,
            EExpr::Trap => return Ok(None),
            EExpr::Tuple(fields) => EType::Tuple(self.values(fields, "a tuple field")?),
            EExpr::Struct { id, name, fields } => {
                let Some(decl) = self.module.structs.iter().find(|decl| decl.id == *id) else {
                    return fail(format!("struct {name} was not emitted"));
                };
                let expected: Vec<EType> = decl.fields.iter().map(|(_, ty)| ty.clone()).collect();
                let values: Vec<EExpr> = fields.iter().map(|(_, value)| value.clone()).collect();
                self.arguments(&values, &expected, &format!("the fields of {name}"))?;
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
                self.arguments(
                    payload,
                    &expected,
                    &format!("the payload of {}", variant.name),
                )?;
                EType::Enum(*id)
            }
            EExpr::Field { target, index, .. } => {
                let fields = match self.value(target, "a projected value")? {
                    EType::Tuple(fields) => fields,
                    EType::Struct(id) => match self.module.structs.iter().find(|d| d.id == id) {
                        Some(decl) => decl.fields.iter().map(|(_, ty)| ty.clone()).collect(),
                        None => return fail("projection from a struct that was not emitted"),
                    },
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
                if !matches!(prim, Prim::WrappingAdd | Prim::WrappingSub) {
                    return fail(format!("{} has no runtime form", prim.name()));
                }
                let mut operands = vec![self.value(receiver, "a receiver")?];
                operands.extend(self.values(arguments, "an argument")?);
                if operands != [EType::U8, EType::U8] {
                    return fail(format!("{} applied to {operands:?}", prim.name()));
                }
                EType::U8
            }
            EExpr::Compare { left, right, .. } => {
                let operands = [
                    self.value(left, "a compared value")?,
                    self.value(right, "a compared value")?,
                ];
                if operands != [EType::U8, EType::U8] {
                    return fail(format!("a comparison of {operands:?}"));
                }
                EType::Bool
            }
            EExpr::Call {
                callee,
                name,
                arguments,
            } => {
                let Some((params, result)) = self.signatures.get(callee).cloned() else {
                    return fail(format!("{name} was not emitted"));
                };
                self.arguments(arguments, &params, &format!("the arguments of {name}"))?;
                result
            }
            EExpr::If {
                condition,
                then_block,
                else_block,
            } => {
                if self.value(condition, "a condition")? != EType::Bool {
                    return fail("a condition that is not a bool");
                }
                let then_type = self.block(then_block)?;
                let else_type = self.block(else_block)?;
                return join(then_type, else_type, "the branches of an if");
            }
            EExpr::Match {
                scrutinee, arms, ..
            } => {
                let EType::Enum(id) = self.value(scrutinee, "a scrutinee")? else {
                    return fail("a match on something that is not an enum");
                };
                let Some(decl) = self.module.enums.iter().find(|decl| decl.id == id) else {
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
                        self.env.push((*id, ty.clone()));
                    }
                    let arm_type = self.block(&arm.body);
                    self.env.truncate(scope);
                    result = join(result, arm_type?, "the arms of a match")?;
                }
                return Ok(result);
            }
            EExpr::Block(block) => return self.block(block),
            EExpr::Loop {
                state,
                result,
                body,
            } => {
                let types = self.state(state)?;
                self.iterate(state, types, Some(result.clone()), body, None)?;
                result.clone()
            }
            EExpr::For {
                index,
                lo,
                hi,
                state,
                body,
            } => {
                for bound in [lo, hi] {
                    if self.value(bound, "a bound")? != EType::U8 {
                        return fail("a for bound that is not a u8");
                    }
                }
                let types = self.state(state)?;
                self.iterate(state, types.clone(), None, body, Some(index.0))?;
                EType::Tuple(types)
            }
            EExpr::Break(value) => {
                let Some(target) = self.targets.last().cloned() else {
                    return fail("break outside a loop");
                };
                let Some(result) = target.result else {
                    return fail("break inside a for");
                };
                let found = self.value(value, "a break value")?;
                if found != result {
                    return fail(format!("break with {found:?}, the loop yields {result:?}"));
                }
                return Ok(None);
            }
            EExpr::Continue(next) => {
                let Some(target) = self.targets.last().cloned() else {
                    return fail("continue outside a loop");
                };
                self.arguments(next, &target.state, "the next loop state")?;
                return Ok(None);
            }
        }))
    }

    /// Checks the body of a loop or a for: it sees the state, and the index
    /// if there is one, and must not yield a value.
    fn iterate(
        &mut self,
        state: &[(VarId, String, EType, EExpr)],
        types: Vec<EType>,
        result: Option<EType>,
        body: &EBlock,
        index: Option<VarId>,
    ) -> Result<(), TypeError> {
        let scope = self.env.len();
        if let Some(index) = index {
            self.env.push((index, EType::U8));
        }
        for (id, _, ty, _) in state {
            self.env.push((*id, ty.clone()));
        }
        self.targets.push(Target {
            state: types,
            result,
        });
        let yielded = self.block(body);
        self.targets.pop();
        self.env.truncate(scope);
        match yielded? {
            None => Ok(()),
            Some(_) => fail("a loop body that falls through"),
        }
    }
}
