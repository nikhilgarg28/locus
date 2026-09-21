//! The reference interpreter for the erased tree. It gives the semantics an
//! executable definition and serves as a test oracle; it is not trusted.
//!
//! Evaluation is left to right and call by value. A call ends in one of
//! three ways, its `Outcome`: it returns a value, it panics with a message,
//! or it runs out of fuel. A fuel counter makes divergence observable: a
//! program that does not return runs out of fuel instead of hanging. Out of
//! fuel is not a fourth behaviour of the program: it says that this run saw
//! neither a value nor a panic within its budget, and nothing more. The
//! meaning of the primitives is the kernel's native evaluation, so logic and
//! execution share one definition.

use std::fmt;

use crate::kernel::{EnumId, Prim, StructId, Term, VarId, evaluate_primitive};
use crate::typed::{CompareOp, FnRef};

use super::tree::{EBlock, EExpr, EPattern, EStmt, Module};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Bool(bool),
    U8(u8),
    Proved,
    Ghost,
    Tuple(Vec<Value>),
    Struct(StructId, Vec<Value>),
    Variant(EnumId, usize, Vec<Value>),
}

/// How a call ended. Both interpreters answer with this type, so comparing
/// them is an equality of outcomes, except that `OutOfFuel` is the absence of
/// an answer and agrees with nothing, itself included.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The call returned.
    Value(Value),
    /// The call panicked, with this message. The panic passed through every
    /// construct that was being evaluated and nothing after it ran.
    Panic(String),
    /// The call did not end within the fuel it was given.
    OutOfFuel,
}

impl Outcome {
    /// The outcome as the compiled program's harness prints it: a value as
    /// Rust's `{:?}` would, a panic as `panic: message`.
    pub fn debug(&self, module: &Module) -> String {
        match self {
            Self::Value(value) => value.debug(module),
            Self::Panic(message) => format!("panic: {message}"),
            Self::OutOfFuel => "out of fuel".into(),
        }
    }
}

/// The interpreter could not run the program. None of these is an outcome of
/// the program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunError {
    /// Calls nested more deeply than the interpreter allows.
    TooDeep,
    /// Control reached a point the program was shown never to reach.
    Trap,
    /// The program is not well formed; the type checker rejects these.
    Stuck(String),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooDeep => f.write_str("calls nested too deeply"),
            Self::Trap => f.write_str("reached a trap"),
            Self::Stuck(why) => write!(f, "stuck: {why}"),
        }
    }
}

impl std::error::Error for RunError {}

/// Why evaluation stopped short of a value. It travels as the error of every
/// step of an interpreter, so `?` passes a panic out through whatever was
/// being evaluated, in evaluation order, just as it passes out of fuel. A
/// `return` travels the same way and stops at the call of its function.
#[derive(Debug)]
pub(crate) enum Stop {
    /// A `return` with its value, on its way to the call it ends.
    Return(Value),
    Panic(String),
    OutOfFuel,
    Error(RunError),
}

impl From<RunError> for Stop {
    fn from(error: RunError) -> Self {
        Self::Error(error)
    }
}

/// What a whole call amounts to: a panic and out of fuel are outcomes, and
/// anything else that stopped it is an error.
pub(crate) fn outcome(result: Result<Value, Stop>) -> Result<Outcome, RunError> {
    match result {
        Ok(value) => Ok(Outcome::Value(value)),
        Err(Stop::Panic(message)) => Ok(Outcome::Panic(message)),
        Err(Stop::OutOfFuel) => Ok(Outcome::OutOfFuel),
        Err(Stop::Error(error)) => Err(error),
        // Every call catches the returns of its own body.
        Err(Stop::Return(_)) => Err(RunError::Stuck("a return outside a function".into())),
    }
}

/// How evaluating an expression ended.
enum Flow {
    Value(Value),
    Break(Value),
    Continue(Vec<Value>),
}

const MAX_CALL_DEPTH: usize = 200;

pub struct Interpreter<'m> {
    module: &'m Module,
    fuel: u64,
    depth: usize,
    env: Vec<(VarId, Value)>,
}

fn stuck<T>(why: impl Into<String>) -> Result<T, Stop> {
    Err(RunError::Stuck(why.into()).into())
}

/// Evaluates a subexpression whose value is needed, passing a control
/// transfer on to whoever handles it.
macro_rules! value {
    ($flow:expr) => {
        match $flow? {
            Flow::Value(value) => value,
            other => return Ok(other),
        }
    };
}

impl<'m> Interpreter<'m> {
    pub fn new(module: &'m Module, fuel: u64) -> Self {
        Self {
            module,
            fuel,
            depth: 0,
            env: Vec::new(),
        }
    }

    pub fn fuel_left(&self) -> u64 {
        self.fuel
    }

    /// Calls a function of the module.
    pub fn call(&mut self, callee: FnRef, arguments: Vec<Value>) -> Result<Outcome, RunError> {
        outcome(self.enter(callee, arguments))
    }

    fn enter(&mut self, callee: FnRef, arguments: Vec<Value>) -> Result<Value, Stop> {
        let Some(function) = self.module.fns.iter().find(|f| f.reference == callee) else {
            return stuck("a call to a function that was not emitted");
        };
        if function.params.len() != arguments.len() {
            return stuck(format!("{} called with the wrong arity", function.name));
        }
        if self.depth >= MAX_CALL_DEPTH {
            return Err(RunError::TooDeep.into());
        }
        // A function body sees its parameters and nothing of its caller.
        let saved = std::mem::take(&mut self.env);
        self.env = function
            .params
            .iter()
            .map(|(id, _, _)| *id)
            .zip(arguments)
            .collect();
        self.depth += 1;
        let result = self.block(&function.body);
        self.depth -= 1;
        self.env = saved;
        match result {
            Ok(Flow::Value(value)) | Err(Stop::Return(value)) => Ok(value),
            Ok(_) => stuck(format!("{} ended in break or continue", function.name)),
            Err(stop) => Err(stop),
        }
    }

    fn spend(&mut self) -> Result<(), Stop> {
        if self.fuel == 0 {
            return Err(Stop::OutOfFuel);
        }
        self.fuel -= 1;
        Ok(())
    }

    fn block(&mut self, block: &EBlock) -> Result<Flow, Stop> {
        let scope = self.env.len();
        let result = self.block_in_scope(block);
        self.env.truncate(scope);
        result
    }

    fn block_in_scope(&mut self, block: &EBlock) -> Result<Flow, Stop> {
        for stmt in &block.stmts {
            match stmt {
                EStmt::Let { pattern, value } => {
                    let value = value!(self.expr(value));
                    self.bind(pattern, value)?;
                }
                EStmt::Expr(expr) => {
                    value!(self.expr(expr));
                }
            }
        }
        match &block.tail {
            Some(tail) => self.expr(tail),
            None => Ok(Flow::Value(Value::Tuple(Vec::new()))),
        }
    }

    fn bind(&mut self, pattern: &EPattern, value: Value) -> Result<(), Stop> {
        match (pattern, value) {
            (EPattern::Wildcard, _) => Ok(()),
            (EPattern::Bind { id, .. }, value) => {
                self.env.push((*id, value));
                Ok(())
            }
            (EPattern::Tuple(patterns), Value::Tuple(fields)) if patterns.len() == fields.len() => {
                patterns
                    .iter()
                    .zip(fields)
                    .try_for_each(|(pattern, field)| self.bind(pattern, field))
            }
            _ => stuck("a tuple pattern against something else"),
        }
    }

    /// Evaluates expressions left to right, stopping at a control transfer.
    fn all(&mut self, exprs: &[EExpr]) -> Result<Result<Vec<Value>, Flow>, Stop> {
        let mut values = Vec::new();
        for expr in exprs {
            match self.expr(expr)? {
                Flow::Value(value) => values.push(value),
                other => return Ok(Err(other)),
            }
        }
        Ok(Ok(values))
    }

    fn expr(&mut self, expr: &EExpr) -> Result<Flow, Stop> {
        self.spend()?;
        Ok(Flow::Value(match expr {
            EExpr::Var { id, name } => match self.env.iter().rev().find(|(var, _)| var == id) {
                Some((_, value)) => value.clone(),
                None => return stuck(format!("{name} is not bound")),
            },
            EExpr::Bool(value) => Value::Bool(*value),
            EExpr::U8(value) => Value::U8(*value),
            EExpr::Proved => Value::Proved,
            EExpr::Ghost => Value::Ghost,
            EExpr::Trap => return Err(RunError::Trap.into()),
            EExpr::Panic { message } => return Err(Stop::Panic(message.clone())),
            EExpr::Tuple(fields) => match self.all(fields)? {
                Ok(values) => Value::Tuple(values),
                Err(flow) => return Ok(flow),
            },
            EExpr::Struct { id, fields, .. } => {
                let exprs: Vec<EExpr> = fields.iter().map(|(_, value)| value.clone()).collect();
                match self.all(&exprs)? {
                    Ok(values) => Value::Struct(*id, values),
                    Err(flow) => return Ok(flow),
                }
            }
            EExpr::Variant {
                id, index, payload, ..
            } => match self.all(payload)? {
                Ok(values) => Value::Variant(*id, *index, values),
                Err(flow) => return Ok(flow),
            },
            EExpr::Field { target, index, .. } => match value!(self.expr(target)) {
                Value::Tuple(fields) | Value::Struct(_, fields) => {
                    match fields.into_iter().nth(*index) {
                        Some(field) => field,
                        None => return stuck("no such field"),
                    }
                }
                _ => return stuck("projection from something that is not a product"),
            },
            EExpr::Method {
                prim,
                receiver,
                arguments,
            } => {
                let receiver = value!(self.expr(receiver));
                let mut operands = vec![receiver];
                match self.all(arguments)? {
                    Ok(values) => operands.extend(values),
                    Err(flow) => return Ok(flow),
                }
                primitive(*prim, &operands)?
            }
            EExpr::Compare { op, left, right } => {
                let left = value!(self.expr(left));
                let right = value!(self.expr(right));
                // The same reading as lowering gives these operators.
                let (prim, operands, negate) = match op {
                    CompareOp::Eq => (Prim::U8Eq, [left, right], false),
                    CompareOp::Ne => (Prim::U8Eq, [left, right], true),
                    CompareOp::Lt => (Prim::U8Lt, [left, right], false),
                    CompareOp::Le => (Prim::U8Le, [left, right], false),
                    CompareOp::Gt => (Prim::U8Lt, [right, left], false),
                    CompareOp::Ge => (Prim::U8Le, [right, left], false),
                };
                match primitive(prim, &operands)? {
                    Value::Bool(result) => Value::Bool(result != negate),
                    _ => return stuck("a comparison that is not a bool"),
                }
            }
            EExpr::Call {
                callee, arguments, ..
            } => match self.all(arguments)? {
                Ok(values) => self.enter(*callee, values)?,
                Err(flow) => return Ok(flow),
            },
            EExpr::If {
                condition,
                then_block,
                else_block,
            } => match value!(self.expr(condition)) {
                Value::Bool(true) => return self.block(then_block),
                Value::Bool(false) => return self.block(else_block),
                _ => return stuck("a condition that is not a bool"),
            },
            EExpr::Match {
                scrutinee, arms, ..
            } => {
                let Value::Variant(_, index, payload) = value!(self.expr(scrutinee)) else {
                    return stuck("a match on something that is not a variant");
                };
                let Some(arm) = arms.get(index) else {
                    return stuck("a match with no arm for the variant");
                };
                if arm.payload.len() != payload.len() {
                    return stuck("an arm that binds the wrong payload");
                }
                let scope = self.env.len();
                let ids = arm.payload.iter().map(|(id, _)| *id);
                self.env.extend(ids.zip(payload));
                let result = self.block(&arm.body);
                self.env.truncate(scope);
                return result;
            }
            EExpr::Block(block) => return self.block(block),
            EExpr::Loop { state, body, .. } => {
                let mut current = match self.initial(state)? {
                    Ok(values) => values,
                    Err(flow) => return Ok(flow),
                };
                loop {
                    self.spend()?;
                    match self.iteration(state, current, None, body)? {
                        Flow::Break(value) => break value,
                        Flow::Continue(next) => current = next,
                        Flow::Value(_) => return stuck("a loop body that falls through"),
                    }
                }
            }
            EExpr::For {
                index,
                lo,
                hi,
                state,
                body,
            } => {
                let (Value::U8(lo), Value::U8(hi)) = (value!(self.expr(lo)), value!(self.expr(hi)))
                else {
                    return stuck("a for bound that is not a u8");
                };
                let mut current = match self.initial(state)? {
                    Ok(values) => values,
                    Err(flow) => return Ok(flow),
                };
                for i in lo..hi {
                    self.spend()?;
                    let at = Some((index.0, Value::U8(i)));
                    match self.iteration(state, current, at, body)? {
                        Flow::Continue(next) => current = next,
                        _ => return stuck("a for body that does not continue"),
                    }
                }
                Value::Tuple(current)
            }
            EExpr::Break(value) => return Ok(Flow::Break(value!(self.expr(value)))),
            EExpr::Continue(next) => {
                return Ok(match self.all(next)? {
                    Ok(values) => Flow::Continue(values),
                    Err(flow) => flow,
                });
            }
            EExpr::Return(value) => return Err(Stop::Return(value!(self.expr(value)))),
        }))
    }

    fn initial(
        &mut self,
        state: &[(VarId, String, super::tree::EType, EExpr)],
    ) -> Result<Result<Vec<Value>, Flow>, Stop> {
        let exprs: Vec<EExpr> = state.iter().map(|(_, _, _, init)| init.clone()).collect();
        self.all(&exprs)
    }

    /// Runs a loop body once with the given state in scope.
    fn iteration(
        &mut self,
        state: &[(VarId, String, super::tree::EType, EExpr)],
        current: Vec<Value>,
        index: Option<(VarId, Value)>,
        body: &EBlock,
    ) -> Result<Flow, Stop> {
        if state.len() != current.len() {
            return stuck("continue with the wrong number of state values");
        }
        let scope = self.env.len();
        self.env.extend(index);
        self.env
            .extend(state.iter().map(|(id, _, _, _)| *id).zip(current));
        let result = self.block(body);
        self.env.truncate(scope);
        result
    }
}

/// The kernel's native evaluation of a primitive, on interpreter values.
fn primitive(prim: Prim, operands: &[Value]) -> Result<Value, Stop> {
    let terms: Option<Vec<Term>> = operands
        .iter()
        .map(|operand| match operand {
            Value::U8(byte) => Some(Term::U8(*byte)),
            Value::Bool(flag) => Some(Term::Bool(*flag)),
            _ => None,
        })
        .collect();
    match terms.and_then(|terms| evaluate_primitive(prim, &terms)) {
        Some(Term::U8(byte)) => Ok(Value::U8(byte)),
        Some(Term::Bool(flag)) => Ok(Value::Bool(flag)),
        _ => stuck(format!("{} has no runtime meaning here", prim.name())),
    }
}
