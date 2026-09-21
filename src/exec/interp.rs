//! An interpreter for the check IR that skips ghosts. It exists for one
//! purpose: to be compared with the interpreter for the erased tree. The
//! checker sees the lowering of a typed tree and the machine runs its
//! erasure; if `lower` and `erase` read the tree the same way, the two
//! interpreters agree on every program. It is not trusted and nothing else
//! uses it.
//!
//! A proof evaluates to `Proved` and anything else with no runtime form to
//! `Ghost`, without being looked into, which is what skipping means here.
//!
//! A call ends in the same `Outcome` as in the other interpreter: a value, a
//! panic with its message, or out of fuel. The check IR has no construct that
//! panics yet, so nothing here produces `Outcome::Panic`; the case is part of
//! the type so that the two interpreters are compared as outcomes from now
//! on, and the construct arrives with the checked semantics of panics.

use crate::erased::{Outcome, RunError, Stop, Value, outcome};
use crate::kernel::{ForLoop, Prim, Term, VarId, evaluate_primitive};
use crate::typed::FnRef;

use super::check::Program;
use super::ir::{Arm, Block, ForStmt, Stmt, Tail};

enum Flow {
    Value(Value),
    Break(Value),
    Continue(Vec<Value>),
}

const MAX_CALL_DEPTH: usize = 200;

pub struct CheckInterpreter<'p> {
    program: &'p Program,
    fuel: u64,
    depth: usize,
    /// Variables bound by statements, by identity.
    free: Vec<(VarId, Value)>,
    /// Variables bound inside kernel terms, innermost last.
    bound: Vec<Value>,
}

fn stuck<T>(why: impl Into<String>) -> Result<T, Stop> {
    Err(RunError::Stuck(why.into()).into())
}

impl<'p> CheckInterpreter<'p> {
    pub fn new(program: &'p Program, fuel: u64) -> Self {
        Self {
            program,
            fuel,
            depth: 0,
            free: Vec::new(),
            bound: Vec::new(),
        }
    }

    pub fn call(&mut self, callee: FnRef, arguments: Vec<Value>) -> Result<Outcome, RunError> {
        outcome(self.enter(callee, arguments))
    }

    fn enter(&mut self, callee: FnRef, arguments: Vec<Value>) -> Result<Value, Stop> {
        if self.depth >= MAX_CALL_DEPTH {
            return Err(RunError::TooDeep.into());
        }
        self.depth += 1;
        let result = match callee {
            FnRef::Math(id) => self.call_math(id, arguments),
            FnRef::Exec(id) => self.call_exec(id, arguments),
        };
        self.depth -= 1;
        result
    }

    fn call_math(&mut self, id: crate::kernel::FnId, arguments: Vec<Value>) -> Result<Value, Stop> {
        let Some((arity, body)) = self.program.definitions().function_body(id) else {
            return stuck("a call to an undeclared math function");
        };
        if arity != arguments.len() {
            return stuck("a math function called with the wrong arity");
        }
        // The body is closed apart from its parameters.
        let saved_bound = std::mem::replace(&mut self.bound, arguments);
        let saved_free = std::mem::take(&mut self.free);
        let result = self.term(body);
        self.bound = saved_bound;
        self.free = saved_free;
        result
    }

    fn call_exec(&mut self, id: super::ir::ExecFnId, arguments: Vec<Value>) -> Result<Value, Stop> {
        let Some(function) = self.program.function(id) else {
            return stuck("a call to an undeclared function");
        };
        if function.params.len() != arguments.len() {
            return stuck("a function called with the wrong arity");
        }
        let saved_free = std::mem::replace(
            &mut self.free,
            function.params.iter().copied().zip(arguments).collect(),
        );
        let saved_bound = std::mem::take(&mut self.bound);
        let result = self.block(&function.body);
        self.free = saved_free;
        self.bound = saved_bound;
        match result? {
            Flow::Value(value) => Ok(value),
            _ => stuck("a function body ended in break or continue"),
        }
    }

    fn spend(&mut self) -> Result<(), Stop> {
        if self.fuel == 0 {
            return Err(Stop::OutOfFuel);
        }
        self.fuel -= 1;
        Ok(())
    }

    // --- Statements -------------------------------------------------------------

    fn block(&mut self, block: &Block) -> Result<Flow, Stop> {
        let scope = self.free.len();
        let result = self.block_in_scope(block);
        self.free.truncate(scope);
        result
    }

    fn block_in_scope(&mut self, block: &Block) -> Result<Flow, Stop> {
        for stmt in &block.stmts {
            if let Some(flow) = self.stmt(stmt)? {
                return Ok(flow);
            }
        }
        match &block.tail {
            Tail::Value(value) => Ok(Flow::Value(self.term(value)?)),
            Tail::Break(value) => Ok(Flow::Break(self.term(value)?)),
            Tail::Continue(next) => Ok(Flow::Continue(self.terms(next)?)),
            Tail::Match { scrutinee, arms } => self.arms(scrutinee, arms),
        }
    }

    /// Runs a statement. `Some` is a control transfer out of a nested arm.
    fn stmt(&mut self, stmt: &Stmt) -> Result<Option<Flow>, Stop> {
        self.spend()?;
        match stmt {
            Stmt::Let { var, value, .. } => {
                let value = self.term(value)?;
                self.free.push((*var, value));
            }
            // A proof made available as a hypothesis: nothing runs.
            Stmt::Have { .. } => {}
            Stmt::Call {
                var,
                callee,
                arguments,
            } => {
                let arguments = self.terms(arguments)?;
                let value = self.enter(FnRef::Exec(*callee), arguments)?;
                self.free.push((*var, value));
            }
            Stmt::Match {
                var,
                scrutinee,
                arms,
                ..
            } => match self.arms(scrutinee, arms)? {
                Flow::Value(value) => self.free.push((*var, value)),
                other => return Ok(Some(other)),
            },
            Stmt::Loop {
                var,
                vars,
                init,
                body,
                ..
            } => {
                let mut current = self.terms(init)?;
                let value = loop {
                    self.spend()?;
                    match self.iteration(vars, current, None, body)? {
                        Flow::Break(value) => break value,
                        Flow::Continue(next) => current = next,
                        Flow::Value(_) => return stuck("a loop body that falls through"),
                    }
                };
                self.free.push((*var, value));
            }
            Stmt::For(looped) => {
                let ForStmt {
                    var,
                    index,
                    lo,
                    hi,
                    vars,
                    init,
                    body,
                    ..
                } = &**looped;
                let (Value::U8(lo), Value::U8(hi)) = (self.term(lo)?, self.term(hi)?) else {
                    return stuck("a for bound that is not a u8");
                };
                let mut current = self.terms(init)?;
                for i in lo..hi {
                    self.spend()?;
                    let at = Some((*index, Value::U8(i)));
                    match self.iteration(vars, current, at, body)? {
                        Flow::Continue(next) => current = next,
                        _ => return stuck("a for body that does not continue"),
                    }
                }
                self.free.push((*var, Value::Tuple(current)));
            }
        }
        Ok(None)
    }

    fn iteration(
        &mut self,
        vars: &[VarId],
        current: Vec<Value>,
        index: Option<(VarId, Value)>,
        body: &Block,
    ) -> Result<Flow, Stop> {
        if vars.len() != current.len() {
            return stuck("continue with the wrong number of state values");
        }
        let scope = self.free.len();
        self.free.extend(index);
        self.free.extend(vars.iter().copied().zip(current));
        let result = self.block(body);
        self.free.truncate(scope);
        result
    }

    fn arms(&mut self, scrutinee: &Term, arms: &[Arm]) -> Result<Flow, Stop> {
        let (index, payload) = match self.term(scrutinee)? {
            Value::Bool(flag) => (usize::from(flag), Vec::new()),
            Value::Variant(_, index, payload) => (index, payload),
            _ => return stuck("a match on something that is not a bool or a variant"),
        };
        let Some(arm) = arms.get(index) else {
            return stuck("a match with no arm for the variant");
        };
        if arm.payload.len() != payload.len() {
            return stuck("an arm that binds the wrong payload");
        }
        let scope = self.free.len();
        self.free.extend(arm.payload.iter().copied().zip(payload));
        let result = self.block(&arm.body);
        self.free.truncate(scope);
        result
    }

    // --- Kernel terms ---------------------------------------------------------------

    fn terms(&mut self, terms: &[Term]) -> Result<Vec<Value>, Stop> {
        terms.iter().map(|term| self.term(term)).collect()
    }

    fn term(&mut self, term: &Term) -> Result<Value, Stop> {
        self.spend()?;
        Ok(match term {
            Term::Free(id) => match self.free.iter().rev().find(|(var, _)| var == id) {
                Some((_, value)) => value.clone(),
                None => return stuck("a variable that is not bound"),
            },
            Term::Bound(index) => {
                let position = self.bound.len().checked_sub(1 + *index as usize);
                match position.and_then(|position| self.bound.get(position)) {
                    Some(value) => value.clone(),
                    None => return stuck("a dangling bound variable"),
                }
            }
            Term::Bool(flag) => Value::Bool(*flag),
            Term::U8(byte) => Value::U8(*byte),
            // Skipped, not evaluated.
            Term::Proof(_) => Value::Proved,
            Term::Nat(_)
            | Term::Eq(..)
            | Term::Implies(..)
            | Term::Forall(..)
            | Term::Exists(..)
            | Term::PropApp(..)
            | Term::Fn(_) => Value::Ghost,
            Term::Absurd(..) => return Err(RunError::Trap.into()),
            Term::Prim(prim, operands) => {
                let operands = self.terms(operands)?;
                primitive(*prim, &operands)?
            }
            Term::Tuple(_, values) => Value::Tuple(self.terms(values)?),
            Term::Struct(id, values) => Value::Struct(*id, self.terms(values)?),
            Term::Variant(id, index, payload) => Value::Variant(*id, *index, self.terms(payload)?),
            Term::Proj(target, index) => match self.term(target)? {
                Value::Tuple(fields) | Value::Struct(_, fields) => {
                    match fields.into_iter().nth(*index) {
                        Some(field) => field,
                        None => return stuck("no such field"),
                    }
                }
                // A projection from something ghost is ghost.
                Value::Ghost => Value::Ghost,
                _ => return stuck("projection from something that is not a product"),
            },
            Term::Call(callee, arguments) => {
                let Term::Fn(id) = &**callee else {
                    return stuck("a call through a function value");
                };
                let arguments = self.terms(arguments)?;
                self.enter(FnRef::Math(*id), arguments)?
            }
            Term::Case {
                scrutinee, arms, ..
            } => {
                let (index, payload) = match self.term(scrutinee)? {
                    Value::Bool(flag) => (usize::from(flag), Vec::new()),
                    Value::Variant(_, index, payload) => (index, payload),
                    _ => return stuck("a case on something that is not a bool or a variant"),
                };
                let Some(arm) = arms.get(index) else {
                    return stuck("a case with no arm for the variant");
                };
                let scope = self.bound.len();
                self.bound.extend(payload);
                let result = self.term(&arm.body);
                self.bound.truncate(scope);
                result?
            }
            Term::For(looped) => self.for_term(looped)?,
        })
    }

    fn for_term(&mut self, looped: &ForLoop) -> Result<Value, Stop> {
        let (Value::U8(lo), Value::U8(hi)) = (self.term(&looped.lo)?, self.term(&looped.hi)?)
        else {
            return stuck("a for bound that is not a u8");
        };
        let mut state = self.term(&looped.init)?;
        for i in lo..hi {
            self.spend()?;
            // The body is under the index and then the state.
            let scope = self.bound.len();
            self.bound.push(Value::U8(i));
            self.bound.push(state);
            let next = self.term(&looped.body);
            self.bound.truncate(scope);
            state = next?;
        }
        Ok(state)
    }
}

/// A primitive with a runtime meaning is evaluated as the kernel evaluates
/// it. One without, or one applied to a ghost, is ghost.
fn primitive(prim: Prim, operands: &[Value]) -> Result<Value, Stop> {
    let terms: Option<Vec<Term>> = operands
        .iter()
        .map(|operand| match operand {
            Value::U8(byte) => Some(Term::U8(*byte)),
            Value::Bool(flag) => Some(Term::Bool(*flag)),
            _ => None,
        })
        .collect();
    Ok(
        match terms.and_then(|terms| evaluate_primitive(prim, &terms)) {
            Some(Term::U8(byte)) => Value::U8(byte),
            Some(Term::Bool(flag)) => Value::Bool(flag),
            _ => Value::Ghost,
        },
    )
}
