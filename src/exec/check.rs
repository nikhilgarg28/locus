//! The exec checker. It walks a function body in order, extending a kernel
//! context as the specification's section 6.3 describes, and asks the kernel
//! to check every pure term and every proof in the context that holds at
//! that point. It checks nothing about termination.

use std::fmt;
use std::rc::Rc;

use crate::kernel::{
    Context, Definitions, KernelError, Mode, Term, Type, case_variants, check_call, check_proof,
    check_type, check_values, infer_term, same_type, telescope_entry, variant_term,
};

use super::ir::{Arm, Block, ExecFn, ExecFnId, ForStmt, Stmt, Tail};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecError {
    /// The kernel rejected a term, a type, or a proof.
    Kernel(KernelError),
    UnknownFunction,
    /// A function's signature is not a function type, or its parameter
    /// identities do not match it.
    BadSignature,
    /// A loop body reached its end: every path must break or continue.
    FallsThrough,
    /// `break` or `continue` with no enclosing loop.
    NoEnclosingLoop,
    /// A match needs a `bool` or an enum, one arm per variant, each binding
    /// exactly its variant's payload.
    BadMatch,
    /// A loop's state identities do not match its state telescope, or a
    /// `for`'s state is not a function from the index to a tuple type.
    BadLoopState,
    /// `break` where the nearest enclosing iteration is a `for`.
    BreakInFor,
    /// A `for` is stated with the prelude's orderings.
    NoPrelude,
}

impl From<KernelError> for ExecError {
    fn from(error: KernelError) -> Self {
        Self::Kernel(error)
    }
}

impl fmt::Display for ExecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kernel(error) => write!(f, "{error}"),
            Self::UnknownFunction => f.write_str("function is not declared"),
            Self::BadSignature => f.write_str("the signature and parameters do not match"),
            Self::FallsThrough => f.write_str("a loop body must end in break or continue"),
            Self::NoEnclosingLoop => f.write_str("break or continue outside a loop"),
            Self::BadMatch => f.write_str("the arms do not match the scrutinee's variants"),
            Self::BadLoopState => f.write_str("the loop state does not match its telescope"),
            Self::BreakInFor => f.write_str("a bounded for has no break"),
            Self::NoPrelude => f.write_str("a bounded for needs the prelude declarations"),
        }
    }
}

impl std::error::Error for ExecError {}

/// Checked ordinary functions over a set of kernel declarations. A function
/// is checked against the functions declared before it, so it cannot call
/// itself, directly or indirectly.
#[derive(Clone, Debug)]
pub struct Program {
    definitions: Definitions,
    fns: Vec<ExecFn>,
}

/// The iteration that `break` and `continue` refer to. `state` is what a
/// `continue` must supply. A `for` has no `break`, so it has no result here.
#[derive(Clone, Copy)]
struct Target<'a> {
    state: &'a Type,
    result: Option<&'a Type>,
}

impl Program {
    pub fn new(definitions: Definitions) -> Self {
        Self {
            definitions,
            fns: Vec::new(),
        }
    }

    pub fn definitions(&self) -> &Definitions {
        &self.definitions
    }

    /// For adding kernel declarations between functions. Declarations only
    /// grow, so functions already accepted stay accepted.
    pub fn definitions_mut(&mut self) -> &mut Definitions {
        &mut self.definitions
    }

    /// The signature of a declared function.
    pub fn signature(&self, id: ExecFnId) -> Option<&Type> {
        self.fns.get(id.0).map(|function| &function.signature)
    }

    /// Checks a function and, if it is accepted, declares it.
    pub fn declare(&mut self, function: ExecFn) -> Result<ExecFnId, ExecError> {
        self.check_fn(&function)?;
        self.fns.push(function);
        Ok(ExecFnId(self.fns.len() - 1))
    }

    fn check_fn(&self, function: &ExecFn) -> Result<(), ExecError> {
        let mut ctx = Context::with_definitions(Rc::new(self.definitions.clone()));
        check_type(&mut ctx, &function.signature)?;
        let Type::Fn(params, _) = &function.signature else {
            return Err(ExecError::BadSignature);
        };
        if params.len() != function.params.len() {
            return Err(ExecError::BadSignature);
        }
        let mut bound: Vec<Term> = Vec::new();
        for (index, id) in function.params.iter().enumerate() {
            let ty = telescope_entry(&function.signature, index, &bound)
                .ok_or(ExecError::BadSignature)?;
            ctx.declare_with(*id, ty, false)?;
            bound.push(Term::var(*id));
        }
        let result = telescope_entry(&function.signature, params.len(), &bound)
            .ok_or(ExecError::BadSignature)?;
        self.check_block(&mut ctx, &function.body, Some(&result), &[])
    }

    /// Checks a block. `expected` is the type its value must have; `None`
    /// means it may not produce a value, as in a loop body. Everything the
    /// block binds leaves scope with it.
    fn check_block(
        &self,
        ctx: &mut Context,
        block: &Block,
        expected: Option<&Type>,
        loops: &[Target<'_>],
    ) -> Result<(), ExecError> {
        let scope = ctx.checkpoint();
        let result = self.check_block_in_scope(ctx, block, expected, loops);
        ctx.rollback(scope);
        result
    }

    fn check_block_in_scope(
        &self,
        ctx: &mut Context,
        block: &Block,
        expected: Option<&Type>,
        loops: &[Target<'_>],
    ) -> Result<(), ExecError> {
        for stmt in &block.stmts {
            self.check_stmt(ctx, stmt, loops)?;
        }
        match &block.tail {
            Tail::Value(value) => {
                let expected = expected.ok_or(ExecError::FallsThrough)?;
                expect(ctx, value, expected)
            }
            Tail::Break(value) => {
                let target = loops.last().ok_or(ExecError::NoEnclosingLoop)?;
                expect(ctx, value, target.result.ok_or(ExecError::BreakInFor)?)
            }
            Tail::Continue(next) => {
                let target = loops.last().ok_or(ExecError::NoEnclosingLoop)?;
                Ok(check_values(ctx, target.state, next, Mode::Executable)?)
            }
            Tail::Match { scrutinee, arms } => {
                self.check_arms(ctx, scrutinee, arms, expected, loops)
            }
        }
    }

    fn check_stmt(
        &self,
        ctx: &mut Context,
        stmt: &Stmt,
        loops: &[Target<'_>],
    ) -> Result<(), ExecError> {
        match stmt {
            Stmt::Let {
                var,
                equation,
                ty,
                value,
            } => {
                let found = ctx.define_with(*var, *equation, value)?;
                match ty {
                    Some(expected) if !same_type(&found, expected) => {
                        Err(ExecError::Kernel(KernelError::TypeMismatch {
                            expected: expected.clone(),
                            found,
                        }))
                    }
                    _ => Ok(()),
                }
            }
            Stmt::Have { hyp, claim, proof } => {
                check_proof(ctx, proof, claim)?;
                Ok(ctx.assume_with(*hyp, claim.clone())?)
            }
            Stmt::Call {
                var,
                callee,
                arguments,
            } => {
                let callee = self.fns.get(callee.0).ok_or(ExecError::UnknownFunction)?;
                let result = check_call(ctx, &callee.signature, arguments, Mode::Executable)?;
                // No defining equation: the result of a call that may not
                // return is not equal to the call in the logic.
                Ok(ctx.declare_with(*var, result, false)?)
            }
            Stmt::Match {
                var,
                ty,
                scrutinee,
                arms,
            } => {
                // The result type is formed outside the arms, so it cannot
                // mention anything an arm binds.
                check_type(ctx, ty)?;
                self.check_arms(ctx, scrutinee, arms, Some(ty), loops)?;
                Ok(ctx.declare_with(*var, ty.clone(), false)?)
            }
            Stmt::Loop {
                var,
                state,
                vars,
                init,
                result,
                body,
            } => {
                // Formed in the outer context: the state is not in scope in
                // the result type.
                check_type(ctx, result)?;
                check_values(ctx, state, init, Mode::Executable)?;
                let scope = ctx.checkpoint();
                let checked = self.check_loop_body(ctx, state, vars, result, body, loops);
                ctx.rollback(scope);
                checked?;
                Ok(ctx.declare_with(*var, result.clone(), false)?)
            }
            Stmt::For(looped) => {
                let ForStmt {
                    var,
                    index,
                    lower,
                    upper,
                    lo,
                    hi,
                    ordered,
                    state,
                    vars,
                    init,
                    body,
                } = &**looped;
                let prelude = self.definitions.prelude().ok_or(ExecError::NoPrelude)?;
                for bound in [lo, hi] {
                    expect(ctx, bound, &Type::U8)?;
                }
                // Ordered bounds make the final index hi.
                check_proof(ctx, ordered, &prelude.u8_le_prop(lo.clone(), hi.clone()))?;
                check_type(ctx, state)?;
                let state_at = |at: &Term| match state {
                    Type::Fn(params, _) if params.as_slice() == [Type::U8] => {
                        telescope_entry(state, 1, std::slice::from_ref(at))
                            .filter(|ty| matches!(ty, Type::Tuple(_)))
                            .ok_or(ExecError::BadLoopState)
                    }
                    _ => Err(ExecError::BadLoopState),
                };
                check_values(ctx, &state_at(lo)?, init, Mode::Executable)?;

                let scope = ctx.checkpoint();
                let checked = (|| {
                    ctx.declare_with(*index, Type::U8, false)?;
                    let i = Term::var(*index);
                    let current = state_at(&i)?;
                    // index < hi, so the successor does not wrap.
                    let next = state_at(&Term::wrapping_add(i.clone(), Term::U8(1)))?;
                    self.declare_state(ctx, &current, vars)?;
                    ctx.assume_with(*lower, prelude.u8_le_prop(lo.clone(), i.clone()))?;
                    ctx.assume_with(*upper, prelude.u8_lt_prop(i, hi.clone()))?;
                    let mut inner = loops.to_vec();
                    inner.push(Target {
                        state: &next,
                        result: None,
                    });
                    self.check_block(ctx, body, None, &inner)
                })();
                ctx.rollback(scope);
                checked?;
                Ok(ctx.declare_with(*var, state_at(hi)?, false)?)
            }
        }
    }

    /// Declares abstract state variables for a state telescope: the body of
    /// an iteration does not know the initial values.
    fn declare_state(
        &self,
        ctx: &mut Context,
        state: &Type,
        vars: &[crate::kernel::VarId],
    ) -> Result<(), ExecError> {
        let Type::Tuple(fields) = state else {
            return Err(ExecError::BadLoopState);
        };
        if fields.len() != vars.len() {
            return Err(ExecError::BadLoopState);
        }
        let mut bound: Vec<Term> = Vec::new();
        for (index, id) in vars.iter().enumerate() {
            let ty = telescope_entry(state, index, &bound).ok_or(ExecError::BadLoopState)?;
            ctx.declare_with(*id, ty, false)?;
            bound.push(Term::var(*id));
        }
        Ok(())
    }

    fn check_loop_body(
        &self,
        ctx: &mut Context,
        state: &Type,
        vars: &[crate::kernel::VarId],
        result: &Type,
        body: &Block,
        loops: &[Target<'_>],
    ) -> Result<(), ExecError> {
        self.declare_state(ctx, state, vars)?;
        let mut inner = loops.to_vec();
        inner.push(Target {
            state,
            result: Some(result),
        });
        self.check_block(ctx, body, None, &inner)
    }

    fn check_arms(
        &self,
        ctx: &mut Context,
        scrutinee: &Term,
        arms: &[Arm],
        expected: Option<&Type>,
        loops: &[Target<'_>],
    ) -> Result<(), ExecError> {
        // Executable mode: a ghost cannot choose a branch.
        let scrutinee_type = infer_term(ctx, scrutinee, Mode::Executable)?;
        let variants = case_variants(ctx, &scrutinee_type).ok_or(ExecError::BadMatch)?;
        if variants.len() != arms.len() {
            return Err(ExecError::BadMatch);
        }
        for (index, (arm, payload)) in arms.iter().zip(&variants).enumerate() {
            if arm.payload.len() != payload.len() {
                return Err(ExecError::BadMatch);
            }
            let scope = ctx.checkpoint();
            let checked = (|| {
                let telescope = Type::Tuple(payload.clone());
                let mut bound: Vec<Term> = Vec::new();
                for (field, id) in arm.payload.iter().enumerate() {
                    let ty =
                        telescope_entry(&telescope, field, &bound).ok_or(ExecError::BadMatch)?;
                    ctx.declare_with(*id, ty, false)?;
                    bound.push(Term::var(*id));
                }
                let fact = Term::eq(
                    scrutinee_type.clone(),
                    scrutinee.clone(),
                    variant_term(&scrutinee_type, index, &arm.payload, payload),
                );
                ctx.assume_with(arm.fact, fact)?;
                self.check_block(ctx, &arm.body, expected, loops)
            })();
            ctx.rollback(scope);
            checked?;
        }
        Ok(())
    }
}

/// A value is executable unless its type is ghost, as a function returning
/// only a proof has.
fn expect(ctx: &mut Context, value: &Term, expected: &Type) -> Result<(), ExecError> {
    let mode = if expected.is_ghost() {
        Mode::Logical
    } else {
        Mode::Executable
    };
    let found = infer_term(ctx, value, mode)?;
    if same_type(&found, expected) {
        Ok(())
    } else {
        Err(ExecError::Kernel(KernelError::TypeMismatch {
            expected: expected.clone(),
            found,
        }))
    }
}
