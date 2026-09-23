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
//!
//! The operators `+`, `-`, `*`, `/`, `%`, and unary minus are where Rust
//! panics, and where its two builds differ, so the interpreter has two
//! modes, `Overflow`. In the default mode, `Checks`, every panic condition
//! of the table in `src/kernel/ops.rs` panics with Rust's message. In
//! `Wrap`, the overflow of `+`, `-`, `*`, and unary minus wraps instead, as
//! a build without overflow checks does, and nothing else changes: `/` and
//! `%` still panic on a zero divisor and on `min / -1`, in every build.
//!
//! A reference is a value here. A `&T` argument passes the value of the
//! place lent, and a `&mut T` argument passes it in and, when the call
//! ends, writes the callee's final value of the parameter back to the
//! place, as Rust's caller finds it there: on a return, and on a panic too,
//! which carries the values of the callee's `&mut` parameters at that
//! moment, so that a write before a panic is neither lost nor half done.
//! `call_lending` reports those values at the top for a Rust harness to be
//! compared with.

use std::fmt;

use crate::kernel::{
    CmpOp, EnumId, Integer, MachineInt, Op, Panic, Prim, StructId, Term, VarId, evaluate_primitive,
};
use crate::typed::{CompareOp, FnRef};

use super::tree::{EBlock, EExpr, EPattern, EPlace, EStmt, Module};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Buffer(Vec<Value>),
    Bool(bool),
    /// A machine integer with its type. An `i128` holds every value of
    /// every type up to 64 bits; the value is always within its type's
    /// range, since every primitive that produces one wraps into it.
    Int(MachineInt, i128),
    Proved,
    Ghost,
    Tuple(Vec<Value>),
    Struct(StructId, Vec<Value>),
    Variant(EnumId, usize, Vec<Value>),
}

impl Value {
    /// A byte.
    pub fn u8(byte: u8) -> Self {
        Self::Int(MachineInt::U8, i128::from(byte))
    }
}

/// How an interpreter treats an operation whose panic condition holds, as
/// Rust's two builds do: `Checks` panics at every condition of the table,
/// `Wrap` wraps the overflow of `+`, `-`, `*`, and unary minus and panics
/// at the rest. Both interpreters take one; the default is `Checks`, the
/// stricter behaviour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Overflow {
    #[default]
    Checks,
    Wrap,
}

impl Overflow {
    /// Both modes, checks first.
    pub const ALL: [Self; 2] = [Self::Checks, Self::Wrap];

    pub fn name(self) -> &'static str {
        match self {
            Self::Checks => "overflow checks on",
            Self::Wrap => "overflow checks off",
        }
    }
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
            Self::TooDeep => write!(
                f,
                "MAX_INTERPRETER_CALL_DEPTH limit of {} was exceeded",
                crate::limits::MAX_INTERPRETER_CALL_DEPTH
            ),
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
    /// A panic with its message and, once it has left a call, the values of
    /// that call's `&mut` parameters at the moment of the panic; the caller
    /// writes them back and passes the panic on with its own.
    Panic {
        message: String,
        lent: Vec<Value>,
    },
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
        Err(Stop::Panic { message, .. }) => Ok(Outcome::Panic(message)),
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
    Continue,
}

use crate::limits::MAX_INTERPRETER_CALL_DEPTH as MAX_CALL_DEPTH;

pub struct Interpreter<'m> {
    module: &'m Module,
    fuel: u64,
    overflow: Overflow,
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
            overflow: Overflow::default(),
            depth: 0,
            env: Vec::new(),
        }
    }

    /// The same interpreter in the given overflow mode.
    pub fn with_overflow(mut self, overflow: Overflow) -> Self {
        self.overflow = overflow;
        self
    }

    pub fn fuel_left(&self) -> u64 {
        self.fuel
    }

    /// Calls a function of the module.
    pub fn call(&mut self, callee: FnRef, arguments: Vec<Value>) -> Result<Outcome, RunError> {
        outcome(self.enter(callee, arguments).map(|(value, _)| value))
    }

    /// Calls a function, and returns beside the outcome the values of its
    /// `&mut` parameters when it ended, on a return or at a panic, in
    /// order: what a caller that lent them sees afterwards.
    pub fn call_lending(
        &mut self,
        callee: FnRef,
        arguments: Vec<Value>,
    ) -> Result<(Outcome, Vec<Value>), RunError> {
        match self.enter(callee, arguments) {
            Ok((value, lent)) => Ok((Outcome::Value(value), lent)),
            Err(Stop::Panic { message, lent }) => Ok((Outcome::Panic(message), lent)),
            Err(other) => outcome(Err(other)).map(|outcome| (outcome, Vec::new())),
        }
    }

    /// Runs a call: its value, with the final values of the callee's
    /// `&mut` parameters, or the stop, which for a panic carries them too.
    fn enter(&mut self, callee: FnRef, arguments: Vec<Value>) -> Result<(Value, Vec<Value>), Stop> {
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
        // The parameters are the first entries of the body's scope, and an
        // assignment to one replaced it there.
        let lent: Vec<Value> = function
            .lent()
            .iter()
            .map(|&index| self.env[index].1.clone())
            .collect();
        self.env = saved;
        match result {
            Ok(Flow::Value(value)) | Err(Stop::Return(value)) => Ok((value, lent)),
            Ok(_) => stuck(format!("{} ended in break or continue", function.name)),
            Err(Stop::Panic { message, .. }) => Err(Stop::Panic { message, lent }),
            Err(stop) => Err(stop),
        }
    }

    /// The value of a place: the binding, or the field of it the path names.
    fn read(&mut self, place: &EPlace) -> Result<Value, Stop> {
        let Some((_, value)) = self.env.iter().rev().find(|(var, _)| *var == place.id) else {
            return stuck(format!("{} is not bound", place.name));
        };
        let mut target = value;
        for (index, _) in &place.path {
            target = match target {
                Value::Tuple(fields) | Value::Struct(_, fields) => match fields.get(*index) {
                    Some(field) => field,
                    None => return stuck("a lent field that is not there"),
                },
                _ => return stuck("a lend into something that is not a product"),
            };
        }
        Ok(target.clone())
    }

    /// After a call, on a return or a panic: the callee's final values of
    /// its `&mut` parameters, written back to the places lent, in order.
    fn write_back(&mut self, arguments: &[EExpr], lent: Vec<Value>) -> Result<(), Stop> {
        let places = arguments.iter().filter_map(|argument| match argument {
            EExpr::Lend {
                mutable: true,
                place,
            } => Some(place),
            _ => None,
        });
        let mut lent = lent.into_iter();
        for place in places {
            let Some(value) = lent.next() else {
                return stuck("a callee reported fewer &mut values than were lent");
            };
            self.assign(place, value)?;
        }
        Ok(())
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
                // The right side in full, then the place.
                EStmt::Assign { place, value } => {
                    let value = value!(self.expr(value));
                    self.assign(place, value)?;
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

    /// Replaces the binding, or the field of it the path names, in place.
    fn assign(&mut self, place: &EPlace, value: Value) -> Result<(), Stop> {
        let Some((_, slot)) = self.env.iter_mut().rev().find(|(var, _)| *var == place.id) else {
            return stuck(format!("{} is not bound", place.name));
        };
        let mut target = slot;
        for (index, _) in &place.path {
            target = match target {
                Value::Tuple(fields) | Value::Struct(_, fields) => match fields.get_mut(*index) {
                    Some(field) => field,
                    None => return stuck("assignment to a field that is not there"),
                },
                _ => return stuck("assignment into something that is not a product"),
            };
        }
        *target = value;
        Ok(())
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
            EExpr::Literal(ty, value) => Value::Int(*ty, *value),
            EExpr::Proved => Value::Proved,
            EExpr::Ghost => Value::Ghost,
            EExpr::Trap => return Err(RunError::Trap.into()),
            EExpr::Panic { form, argument } => {
                return Err(Stop::Panic {
                    message: form.message(argument.as_deref()),
                    lent: Vec::new(),
                });
            }
            // Checked in both modes: the builds the modes stand for differ
            // in overflow checks alone, and both have debug assertions on.
            EExpr::Assert {
                condition, message, ..
            } => match value!(self.expr(condition)) {
                Value::Bool(true) => Value::Tuple(Vec::new()),
                Value::Bool(false) => {
                    return Err(Stop::Panic {
                        message: message.clone(),
                        lent: Vec::new(),
                    });
                }
                _ => return stuck("an assertion of something that is not a bool"),
            },
            EExpr::BoxNew(value) => Value::Tuple(vec![value!(self.expr(value))]),
            EExpr::BoxDeref(value) => match value!(self.expr(value)) {
                Value::Tuple(mut fields) if fields.len() == 1 => fields.remove(0),
                _ => return stuck("box dereference expects an owned box"),
            },
            EExpr::Shared { value, .. } | EExpr::Deref(value) => value!(self.expr(value)),
            EExpr::Buffer { op, arguments, .. } => {
                let values = match self.all(arguments)? {
                    Ok(values) => values,
                    Err(flow) => return Ok(flow),
                };
                let result = buffer_operation(*op, &values)?;
                if matches!(
                    op,
                    crate::kernel::BufferOp::Set | crate::kernel::BufferOp::Push
                ) {
                    let Some(EExpr::Lend {
                        mutable: true,
                        place,
                    }) = arguments.first()
                    else {
                        return stuck("buffer mutation without mutable receiver");
                    };
                    self.assign(place, result)?;
                    Value::Tuple(Vec::new())
                } else {
                    result
                }
            }
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
                Value::Bool(compare(*op, left, right)?)
            }
            EExpr::Operate { op, ty, operands } => match self.all(operands)? {
                Ok(values) => operate(self.overflow, *op, *ty, &values)?,
                Err(flow) => return Ok(flow),
            },
            EExpr::Cast { expr, to } => match value!(self.expr(expr)) {
                Value::Int(from, value) => {
                    let term = Term::cast(from, *to, Term::machine_int(from, value));
                    match term_value(term) {
                        Some(value) => value,
                        None => return stuck("a cast that has no value"),
                    }
                }
                _ => return stuck("a cast of something that is not a machine integer"),
            },
            // The arguments in order, a lend reading its place; then the
            // call, and its `&mut` places written back, whether it returned
            // or panicked.
            EExpr::Call {
                callee, arguments, ..
            } => match self.all(arguments)? {
                Ok(values) => match self.enter(*callee, values) {
                    Ok((value, lent)) => {
                        self.write_back(arguments, lent)?;
                        value
                    }
                    Err(Stop::Panic { message, lent }) => {
                        self.write_back(arguments, lent)?;
                        return Err(Stop::Panic {
                            message,
                            lent: Vec::new(),
                        });
                    }
                    Err(stop) => return Err(stop),
                },
                Err(flow) => return Ok(flow),
            },
            EExpr::Lend { place, .. } => self.read(place)?,
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
            // What a loop carries is assigned in place; a pass that reaches
            // the end of the body, or `continue`, starts the next.
            EExpr::Loop { body, .. } => loop {
                self.spend()?;
                if let Flow::Break(value) = self.block(body)? {
                    break value;
                }
            },
            EExpr::While { condition, body } => loop {
                self.spend()?;
                match value!(self.expr(condition)) {
                    Value::Bool(true) => {}
                    Value::Bool(false) => break Value::Tuple(Vec::new()),
                    _ => return stuck("a while condition that is not a bool"),
                }
                if let Flow::Break(value) = self.block(body)? {
                    break value;
                }
            },
            EExpr::For {
                index,
                lo,
                hi,
                inclusive,
                body,
            } => {
                let (Value::Int(ty, lo), Value::Int(hi_type, hi)) =
                    (value!(self.expr(lo)), value!(self.expr(hi)))
                else {
                    return stuck("a for bound that is not a machine integer");
                };
                if ty != hi_type {
                    return stuck("for bounds of two types");
                }
                let last = if *inclusive { hi } else { hi - 1 };
                let mut i = lo;
                loop {
                    if i > last {
                        break Value::Tuple(Vec::new());
                    }
                    self.spend()?;
                    let scope = self.env.len();
                    self.env.push((index.0, Value::Int(ty, i)));
                    let flow = self.block(body);
                    self.env.truncate(scope);
                    if let Flow::Break(value) = flow? {
                        break value;
                    }
                    i += 1;
                }
            }
            EExpr::Break(value) => {
                return Ok(Flow::Break(match value {
                    Some(value) => value!(self.expr(value)),
                    None => Value::Tuple(Vec::new()),
                }));
            }
            EExpr::Continue => return Ok(Flow::Continue),
            EExpr::Return(value) => return Err(Stop::Return(value!(self.expr(value)))),
        }))
    }
}

/// A runtime value as the kernel literal it is, when it is one.
pub(crate) fn value_term(value: &Value) -> Option<Term> {
    match value {
        Value::Int(ty, value) => Some(Term::machine_int(*ty, *value)),
        Value::Bool(flag) => Some(Term::Bool(*flag)),
        _ => None,
    }
}

/// The kernel's native evaluation of a closed primitive application, as a
/// runtime value, when it has one.
pub(crate) fn term_value(term: Term) -> Option<Value> {
    let Term::Prim(prim, operands) = term else {
        return None;
    };
    match evaluate_primitive(prim, &operands)? {
        Term::Bool(flag) => Some(Value::Bool(flag)),
        literal => literal
            .machine_value()
            .map(|(ty, value)| Value::Int(ty, value.to_i128().expect("at most 64 bits"))),
    }
}

/// The kernel's native evaluation of a primitive, on interpreter values.
fn primitive(prim: Prim, operands: &[Value]) -> Result<Value, Stop> {
    let terms: Option<Vec<Term>> = operands.iter().map(value_term).collect();
    match terms.and_then(|terms| term_value(Term::prim(prim, terms))) {
        Some(value) => Ok(value),
        None => stuck(format!("{} has no runtime meaning here", prim.name())),
    }
}

/// An operator of the table applied at runtime, in the given mode: the
/// operands must be values of the row's type; where the row's panic
/// condition holds (`Row::fits_at`) the result is a panic with Rust's
/// message, unless the mode is `Wrap` and the row wraps in a build without
/// overflow checks (`Row::wraps_instead`); otherwise the value is the
/// row's meaning, `Row::compute`, which the kernel evaluates the same.
pub(crate) fn operate(
    overflow: Overflow,
    op: Op,
    ty: MachineInt,
    operands: &[Value],
) -> Result<Value, Stop> {
    let Some(row) = op.row(ty) else {
        return stuck(format!("{} has no row at {}", op.name(), ty.name()));
    };
    let mut numbers = Vec::new();
    for operand in operands {
        match operand {
            Value::Int(found, value) if *found == ty => numbers.push(Integer::from(*value)),
            _ => return stuck(format!("{} applied to something else", row.applied(&[]))),
        }
    }
    if numbers.len() != row.arity() {
        return stuck(format!(
            "{} applied to {} operands",
            op.name(),
            numbers.len()
        ));
    }
    if !row.fits_at(&numbers) && (overflow == Overflow::Checks || !row.wraps_instead()) {
        return Err(Stop::Panic {
            message: panic_message(op, &numbers).into(),
            lent: Vec::new(),
        });
    }
    let value = row.compute(&numbers);
    Ok(Value::Int(ty, value.to_i128().expect("at most 64 bits")))
}

/// Rust's message for the panic of an operator: for `/` and `%`, one for a
/// zero divisor and one for `min / -1`, which Rust calls an overflow.
fn panic_message(op: Op, operands: &[Integer]) -> &'static str {
    let by_zero = op.panic() == Panic::Division && operands[1].is_zero();
    match (op, by_zero) {
        (Op::Add, _) => "attempt to add with overflow",
        (Op::Sub, _) => "attempt to subtract with overflow",
        (Op::Mul, _) => "attempt to multiply with overflow",
        (Op::Neg, _) => "attempt to negate with overflow",
        (Op::Div, true) => "attempt to divide by zero",
        (Op::Div, false) => "attempt to divide with overflow",
        (Op::Rem, true) => "attempt to calculate the remainder with a divisor of zero",
        (Op::Rem, false) => "attempt to calculate the remainder with overflow",
        _ => unreachable!("the wrapping methods never panic"),
    }
}

/// A comparison of two runtime values of one type, read as lowering reads
/// the operator: at a machine type through the kernel's `eq[T]`, `lt[T]`,
/// and `le[T]`, with `>` and `>=` as their flips and `!=` as the negation
/// of `==`; at `bool`, `==` and `!=` only.
pub(crate) fn compare(op: CompareOp, left: Value, right: Value) -> Result<bool, Stop> {
    let (cmp, operands, negate) = match op {
        CompareOp::Eq => (CmpOp::Eq, [left, right], false),
        CompareOp::Ne => (CmpOp::Eq, [left, right], true),
        CompareOp::Lt => (CmpOp::Lt, [left, right], false),
        CompareOp::Le => (CmpOp::Le, [left, right], false),
        CompareOp::Gt => (CmpOp::Lt, [right, left], false),
        CompareOp::Ge => (CmpOp::Le, [right, left], false),
    };
    let result = match &operands {
        [Value::Int(a_type, a), Value::Int(b_type, b)] => {
            if a_type != b_type {
                return stuck("a comparison of two machine types");
            }
            let comparison = Term::cmp(
                cmp,
                *a_type,
                Term::machine_int(*a_type, *a),
                Term::machine_int(*b_type, *b),
            );
            match term_value(comparison) {
                Some(Value::Bool(result)) => result,
                _ => return stuck("a comparison that is not a bool"),
            }
        }
        [Value::Bool(a), Value::Bool(b)] if cmp == CmpOp::Eq => a == b,
        _ => return stuck("a comparison of values that have none"),
    };
    Ok(result != negate)
}

/// Native contents operations, separate from logical Buffer term evaluation.
pub(crate) fn buffer_operation(
    op: crate::kernel::BufferOp,
    arguments: &[Value],
) -> Result<Value, Stop> {
    use crate::kernel::BufferOp;
    if op == BufferOp::Literal {
        return Ok(Value::Buffer(arguments.to_vec()));
    }
    let Some(Value::Buffer(items)) = arguments.first() else {
        return stuck("collection operation needs a buffer");
    };
    match op {
        BufferOp::Length => Ok(Value::Int(MachineInt::U64, items.len() as i128)),
        BufferOp::Get | BufferOp::Set => {
            let Some(Value::Int(MachineInt::U64, index)) = arguments.get(1) else {
                return stuck("collection index is not u64");
            };
            let index = usize::try_from(*index).map_err(|_| Stop::Panic {
                message: "index out of bounds".into(),
                lent: Vec::new(),
            })?;
            if index >= items.len() {
                return Err(Stop::Panic {
                    message: "index out of bounds".into(),
                    lent: Vec::new(),
                });
            }
            if op == BufferOp::Get {
                Ok(items[index].clone())
            } else {
                let mut updated = items.clone();
                updated[index] = arguments
                    .get(2)
                    .ok_or_else(|| Stop::from(RunError::Stuck("missing collection value".into())))?
                    .clone();
                Ok(Value::Buffer(updated))
            }
        }
        BufferOp::Push => {
            let mut updated = items.clone();
            updated.push(
                arguments
                    .get(1)
                    .ok_or_else(|| Stop::from(RunError::Stuck("missing pushed value".into())))?
                    .clone(),
            );
            Ok(Value::Buffer(updated))
        }
        BufferOp::Literal => unreachable!(),
    }
}
