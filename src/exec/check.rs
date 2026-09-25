//! The exec checker. It walks a function body in order, extending a kernel
//! context as the specification's section 6.3 describes, and asks the kernel
//! to check every pure term and every proof in the context that holds at
//! that point. A function is checked for partial correctness, and for each
//! promise it makes: `terminates`, `no_panic`, `no_alloc`, `no_io`.

use std::fmt;
use std::rc::Rc;

use crate::kernel::derive::symm_at;
use crate::kernel::{
    Axiom, Context, Definitions, KernelError, MachineInt, Mode, Op, Panic, Proof, Term, Type,
    case_variants, check_call, check_proof, check_type, check_values, infer_term, same_type,
    telescope_entry, variant_term,
};

use super::ir::{
    Arm, Block, ExecFn, ExecFnId, ForStmt, OperateStmt, Promise, Promises, Stmt, Tail,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecError {
    InvalidForeign,
    InvalidBuffer(&'static str),
    /// The kernel rejected a term, a type, or a proof.
    Kernel(KernelError),
    UnknownFunction,
    /// A function's signature is not a function type, or its parameter
    /// identities do not match it.
    BadSignature,
    /// A loop body reached its end: every path must break, continue,
    /// return, or panic.
    FallsThrough,
    /// `break` or `continue` with no enclosing loop.
    NoEnclosingLoop,
    /// A match needs a `bool` or an enum, one arm per variant, each binding
    /// exactly its variant's payload.
    BadMatch,
    /// A loop's state identities do not match its state telescope.
    BadLoopState,
    /// A `for` is stated with the prelude's orderings, and a panic is shown
    /// unreachable by a proof of the prelude's `False`.
    NoPrelude,
    /// The function makes this promise and calls a function that does not.
    CalleeBreaksPromise {
        promise: Promise,
        callee: ExecFnId,
    },
    /// The function promises `terminates` and contains a loop or a `for`.
    LoopUnderTerminates,
    /// The function promises `no_panic` and has a panic ending with no proof
    /// that it is unreachable.
    PanicUnderNoPanic,
    /// The function promises `no_panic` and applies an operator that may
    /// panic, `+`, `-`, `*`, `/`, `%`, or unary minus at a machine type,
    /// with no evidence that it does not.
    OperationUnderNoPanic {
        op: Op,
        ty: MachineInt,
    },
    /// An operation statement is not the shape its row asks for: the wrong
    /// number of arguments, of proofs in `fits`, or of learned hypotheses.
    BadOperation {
        op: Op,
        ty: MachineInt,
    },
}

impl From<KernelError> for ExecError {
    fn from(error: KernelError) -> Self {
        Self::Kernel(error)
    }
}

impl fmt::Display for ExecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidForeign => write!(f, "native calls require physical scalar/tuple types and make no effect promises"),
            Self::InvalidBuffer(reason) => write!(f, "invalid collection operation: {reason}"),
            Self::Kernel(error) => write!(f, "{error}"),
            Self::UnknownFunction => f.write_str("function is not declared"),
            Self::BadSignature => f.write_str("the signature and parameters do not match"),
            Self::FallsThrough => {
                f.write_str("a loop body must end in break, continue, return, or a panic")
            }
            Self::NoEnclosingLoop => f.write_str("break or continue outside a loop"),
            Self::BadMatch => f.write_str("the arms do not match the scrutinee's variants"),
            Self::BadLoopState => f.write_str("the loop state does not match its telescope"),
            Self::NoPrelude => f.write_str(
                "a bounded for and an unreachable panic need the prelude declarations",
            ),
            Self::CalleeBreaksPromise { promise, callee } => write!(
                f,
                "a function that promises {} calls function {}, which does not",
                promise.name(),
                callee.0
            ),
            Self::LoopUnderTerminates => {
                f.write_str("a function that promises terminates contains a loop")
            }
            Self::PanicUnderNoPanic => f.write_str(
                "a function that promises no_panic has a panic with no proof that it is unreachable",
            ),
            Self::OperationUnderNoPanic { op, ty } => write!(
                f,
                "a function that promises no_panic applies `{}` at {}, which may panic, without evidence that it does not",
                op.symbol(),
                ty.name()
            ),
            Self::BadOperation { op, ty } => write!(
                f,
                "the statement for {}[{}] does not have the shape its row asks for",
                op.name(),
                ty.name()
            ),
        }
    }
}

impl std::error::Error for ExecError {}

/// Checked ordinary functions over a set of kernel declarations. A function
/// is checked against the functions declared before it, so it cannot call
/// itself, directly or indirectly.
///
/// A function is also checked for each promise it makes, so that a promise
/// of a declared function can be relied on:
///
/// - `no_panic`: every callee promises it, every panic ending carries a
///   proof of `False` in the context of its point, and every primitive
///   operation that may panic carries the evidence that it does not
///   (`check_operate`).
/// - `terminates`: every callee promises it, and the body contains no loop
///   and no `for`, however deeply nested. With no recursion, that leaves
///   nothing that can run forever.
/// - `no_alloc` and `no_io`: every callee promises the same. The check IR has
///   no primitive that allocates or performs I/O.
#[derive(Clone, Debug)]
pub struct Program {
    definitions: Definitions,
    fns: Vec<ExecFn>,
    trusted: Vec<TrustedContract>,
}

/// What the function being checked declares, as every point of its body
/// sees it: the result type, which a `return` must supply, and the promises.
/// The result type is formed in the context of the parameters, and a context
/// only grows and never rebinds an identity, so it means the same at every
/// point of the body.
#[derive(Clone, Copy)]
struct Declared<'a> {
    result: &'a Type,
    promises: Promises,
}

/// The iteration that `break` and `continue` refer to. `state` is what a
/// `continue` must supply and `result` what a `break` must supply; in a
/// `for` the two are the same type.
#[derive(Clone, Copy)]
struct Target<'a> {
    state: &'a Type,
    result: &'a Type,
}

/// An explicitly assumed foreign contract, kept available to the audit.
#[derive(Clone, Debug)]
pub struct TrustedContract {
    pub function: ExecFnId,
    pub backend: ExecFnId,
    pub reason: String,
    pub implementation: String,
}

impl Program {
    pub fn new(definitions: Definitions) -> Self {
        Self {
            definitions,
            fns: Vec::new(),
            trusted: Vec::new(),
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

    /// A declared function, for an interpreter.
    pub fn function(&self, id: ExecFnId) -> Option<&ExecFn> {
        self.fns.get(id.0)
    }

    /// The promises of a declared function, each of which was enforced when
    /// it was declared.
    pub fn promises(&self, id: ExecFnId) -> Option<Promises> {
        self.fns.get(id.0).map(|function| function.promises)
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

    pub fn trusted_contracts(&self) -> &[TrustedContract] {
        &self.trusted
    }

    /// The only unchecked function-contract boundary. It requires an already
    /// checked runtime backend, a well-formed header and an attached reason.
    /// No kernel function/axiom is declared: the guarantee is conditional on
    /// this recorded foreign specification and holds only on normal return.
    pub(crate) fn declare_trusted_adapter(
        &mut self,
        backend: ExecFnId,
        signature: Type,
        promises: Promises,
        reason: String,
        implementation: String,
    ) -> Result<ExecFnId, ExecError> {
        if reason.trim().is_empty() || implementation.trim().is_empty() {
            return Err(ExecError::InvalidBuffer(
                "trusted contracts require a reason and implementation",
            ));
        }
        let mut function = self
            .function(backend)
            .cloned()
            .ok_or(ExecError::UnknownFunction)?;
        let mut ctx = Context::with_definitions(Rc::new(self.definitions.clone()));
        check_type(&mut ctx, &signature)?;
        let Type::Fn(params, _) = &signature else {
            return Err(ExecError::BadSignature);
        };
        if params.len() != function.params.len() {
            return Err(ExecError::BadSignature);
        }
        function.signature = signature;
        function.promises = promises;
        let id = ExecFnId(self.fns.len());
        self.fns.push(function);
        self.trusted.push(TrustedContract {
            function: id,
            backend,
            reason,
            implementation,
        });
        Ok(id)
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
        let declared = Declared {
            result: &result,
            promises: function.promises,
        };
        self.check_block(&mut ctx, &function.body, Some(&result), declared, &[])
    }

    /// Checks a block. `expected` is the type its value must have; `None`
    /// means it may not produce a value, as in a loop body. Everything the
    /// block binds leaves scope with it.
    fn check_block(
        &self,
        ctx: &mut Context,
        block: &Block,
        expected: Option<&Type>,
        declared: Declared<'_>,
        loops: &[Target<'_>],
    ) -> Result<(), ExecError> {
        let scope = ctx.checkpoint();
        let result = self.check_block_in_scope(ctx, block, expected, declared, loops);
        ctx.rollback(scope);
        result
    }

    fn check_block_in_scope(
        &self,
        ctx: &mut Context,
        block: &Block,
        expected: Option<&Type>,
        declared: Declared<'_>,
        loops: &[Target<'_>],
    ) -> Result<(), ExecError> {
        for stmt in &block.stmts {
            self.check_stmt(ctx, stmt, declared, loops)?;
        }
        match &block.tail {
            Tail::Foreign {
                arguments, result, ..
            } => {
                if declared.promises != Promises::default()
                    || !foreign_type(result)
                    || expected.is_none_or(|e| !same_type(e, result))
                {
                    return Err(ExecError::InvalidForeign);
                }
                for argument in arguments {
                    if !foreign_type(&infer_term(ctx, argument, Mode::Executable)?) {
                        return Err(ExecError::InvalidForeign);
                    }
                }
                check_type(ctx, result)?;
                Ok(())
            }
            Tail::Value(value) => {
                let expected = expected.ok_or(ExecError::FallsThrough)?;
                expect(ctx, value, expected, &self.definitions)
            }
            Tail::Break(value) => {
                let target = loops.last().ok_or(ExecError::NoEnclosingLoop)?;
                expect(ctx, value, target.result, &self.definitions)
            }
            Tail::Continue(next) => {
                let target = loops.last().ok_or(ExecError::NoEnclosingLoop)?;
                Ok(check_values(ctx, target.state, next, Mode::Executable)?)
            }
            Tail::Match { scrutinee, arms } => {
                self.check_arms(ctx, scrutinee, arms, expected, declared, loops)
            }
            // Whatever the block was expected to produce, it produces
            // nothing: control leaves the function.
            Tail::Return(value) => expect(ctx, value, declared.result, &self.definitions),
            Tail::Panic { unreachable, .. } => match unreachable {
                Some(proof) => {
                    let prelude = self.definitions.prelude().ok_or(ExecError::NoPrelude)?;
                    Ok(check_proof(ctx, proof, &prelude.falsehood_prop())?)
                }
                None if declared.promises.no_panic => Err(ExecError::PanicUnderNoPanic),
                None => Ok(()),
            },
        }
    }

    fn check_stmt(
        &self,
        ctx: &mut Context,
        stmt: &Stmt,
        declared: Declared<'_>,
        loops: &[Target<'_>],
    ) -> Result<(), ExecError> {
        match stmt {
            Stmt::BoxNew {
                var,
                equation,
                value,
                logical_payload,
            } => {
                if declared.promises.no_alloc || declared.promises.no_panic {
                    return Err(ExecError::InvalidBuffer(
                        "box allocation violates a no_alloc/no_panic promise",
                    ));
                }
                let element = infer_term(ctx, value, Mode::Logical)?;
                if *logical_payload
                    && !self.definitions.is_erased_type(&element)
                    && element != Type::Bool
                {
                    return Err(ExecError::InvalidBuffer(
                        "physical box payload cannot be silently erased",
                    ));
                }
                if !*logical_payload && !self.definitions.is_erased_type(&element) {
                    infer_term(ctx, value, Mode::Executable)?;
                }
                let ty = Type::Boxed(Box::new(element));
                let snapshot = Term::Boxed(Box::new(value.clone()));
                infer_term(ctx, &snapshot, Mode::Logical)?;
                ctx.declare_with(*var, ty.clone(), false)?;
                ctx.assume_with(*equation, Term::eq(ty, Term::Free(*var), snapshot))?;
                Ok(())
            }
            Stmt::Buffer(operation) => {
                super::buffer::check(ctx, operation, declared.promises, &self.definitions)
            }
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
                let id = *callee;
                let callee = self.fns.get(id.0).ok_or(ExecError::UnknownFunction)?;
                // A primitive that allocates or performs I/O would be
                // refused here, under `no_alloc` or `no_io`; there is none.
                for promise in Promise::ALL {
                    if declared.promises.makes(promise) && !callee.promises.makes(promise) {
                        return Err(ExecError::CalleeBreaksPromise {
                            promise,
                            callee: id,
                        });
                    }
                }
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
                self.check_arms(ctx, scrutinee, arms, Some(ty), declared, loops)?;
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
                if declared.promises.terminates {
                    return Err(ExecError::LoopUnderTerminates);
                }
                // Formed in the outer context: the state is not in scope in
                // the result type.
                check_type(ctx, result)?;
                if !matches!(state, Type::Tuple(_)) {
                    return Err(ExecError::BadLoopState);
                }
                check_values(ctx, state, init, Mode::Executable)?;
                let scope = ctx.checkpoint();
                let checked = (|| {
                    self.declare_state(ctx, state, vars)?;
                    let mut inner = loops.to_vec();
                    inner.push(Target { state, result });
                    self.check_block(ctx, body, None, declared, &inner)
                })();
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
                    inclusive,
                    state,
                    vars,
                    init,
                    body,
                } = &**looped;
                // A `for` is bounded, but the rule is the design's: under
                // `terminates` there is no iteration of either kind.
                if declared.promises.terminates {
                    return Err(ExecError::LoopUnderTerminates);
                }
                self.definitions.prelude().ok_or(ExecError::NoPrelude)?;
                // The index type is the lower bound's, a machine type, and
                // the upper bound has it too. A ghost cannot decide how many
                // times executable code runs.
                let found = infer_term(ctx, lo, Mode::Executable)?;
                let index_type = found.as_machine().ok_or(KernelError::TypeMismatch {
                    expected: Type::U8,
                    found,
                })?;
                expect(ctx, hi, &Type::machine(index_type), &self.definitions)?;
                let view = |x: &Term| Term::view(index_type, x.clone());
                // The state is a tuple telescope formed outside the loop,
                // as a `loop`'s is.
                check_type(ctx, state)?;
                if !matches!(state, Type::Tuple(_)) {
                    return Err(ExecError::BadLoopState);
                }
                check_values(ctx, state, init, Mode::Executable)?;

                let scope = ctx.checkpoint();
                let checked = (|| {
                    ctx.declare_with(*index, Type::machine(index_type), false)?;
                    let i = Term::var(*index);
                    self.declare_state(ctx, state, vars)?;
                    ctx.assume_with(*lower, Term::int_le(view(lo), view(&i)))?;
                    let below = if *inclusive {
                        Term::int_le(view(&i), view(hi))
                    } else {
                        Term::int_lt(view(&i), view(hi))
                    };
                    ctx.assume_with(*upper, below)?;
                    let mut inner = loops.to_vec();
                    inner.push(Target {
                        state,
                        result: state,
                    });
                    self.check_block(ctx, body, None, declared, &inner)
                })();
                ctx.rollback(scope);
                checked?;
                Ok(ctx.declare_with(*var, state.clone(), false)?)
            }
            Stmt::Operate(operation) => self.check_operate(ctx, operation, declared),
        }
    }

    /// A primitive operation that may panic, `let var = op[ty](arguments)`.
    ///
    /// Operands and any `fits` evidence are checked before binding the result.
    /// `no_panic` requires that evidence. Normal continuation knows the exact
    /// result of checked arithmetic, even when no static safety proof exists;
    /// the runtime check supplies this operational fact. The kernel's total
    /// wrapped equation is also valid whenever execution returns. Division
    /// and remainder additionally establish the failed-panic exclusions.
    fn check_operate(
        &self,
        ctx: &mut Context,
        operation: &OperateStmt,
        declared: Declared<'_>,
    ) -> Result<(), ExecError> {
        let OperateStmt {
            var,
            equation,
            op,
            ty,
            arguments,
            fits,
            learned,
        } = operation;
        let (op, ty) = (*op, *ty);
        let bad = || ExecError::BadOperation { op, ty };
        let row = op
            .row(ty)
            .ok_or(ExecError::Kernel(KernelError::NoRow(op, ty)))?;
        if arguments.len() != row.arity() {
            return Err(bad());
        }
        let prelude = self.definitions.prelude().ok_or(ExecError::NoPrelude)?;
        for argument in arguments {
            expect(ctx, argument, &Type::machine(ty), &self.definitions)?;
        }
        let premises = row.fits(&prelude, arguments);
        let exact_learned =
            row.panic() == Panic::Overflow && (fits.is_some() || !learned.is_empty());
        let expected_learned = match row.panic() {
            Panic::Never => 0,
            Panic::Overflow => usize::from(exact_learned),
            Panic::Division => premises.len(),
        };
        if learned.len() != expected_learned {
            return Err(bad());
        }
        if fits.is_none() && declared.promises.no_panic && row.panic() != Panic::Never {
            return Err(ExecError::OperationUnderNoPanic { op, ty });
        }
        if let Some(proofs) = fits {
            if proofs.len() != premises.len() {
                return Err(bad());
            }
            for (proof, premise) in proofs.iter().zip(&premises) {
                check_proof(ctx, proof, premise)?;
            }
        }
        // Check all input certificates before introducing the result or any
        // successful-continuation facts: they cannot justify their own check.
        ctx.define_with(*var, *equation, &row.applied(arguments))?;
        if let Some(proofs) = fits
            && exact_learned
        {
            // op_exact: min <= e => (e <= max => view(op(xs)) == e); the
            // two premises discharge it, and the equation moves the
            // view from the applied row to `var`.
            let [lower, upper] = proofs.as_slice() else {
                return Err(bad());
            };
            let of_row = Proof::implies_elim(
                Proof::implies_elim(
                    Proof::Axiom(Axiom::OpExact(op, ty, arguments.clone())),
                    lower.clone(),
                ),
                upper.clone(),
            );
            let exact = row.exact_term(arguments);
            let claim = Term::eq(Type::Int, Term::view(ty, Term::var(*var)), exact.clone());
            let proof = Proof::Transport {
                eq: Box::new(symm_at(
                    &Type::machine(ty),
                    &Term::var(*var),
                    Proof::hyp(*equation),
                )),
                template: Term::eq(Type::Int, Term::view(ty, Term::Bound(0)), exact),
                proof: Box::new(of_row),
            };
            check_proof(ctx, &proof, &claim)?;
            ctx.assume_with(learned[0], claim)?;
        }
        if exact_learned && fits.is_none() {
            // Operational rule, like the continuation of assert!: execution
            // reaches here only if the checked exact result fits its type.
            // This is NOT a kernel axiom available to pure logical terms.
            ctx.assume_with(
                learned[0],
                Term::eq(
                    Type::Int,
                    Term::view(ty, Term::var(*var)),
                    row.exact_term(arguments),
                ),
            )?;
        }
        if row.panic() == Panic::Division {
            for (hyp, premise) in learned.iter().zip(premises) {
                ctx.assume_with(*hyp, premise)?;
            }
        }
        Ok(())
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

    fn check_arms(
        &self,
        ctx: &mut Context,
        scrutinee: &Term,
        arms: &[Arm],
        expected: Option<&Type>,
        declared: Declared<'_>,
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
                self.check_block(ctx, &arm.body, expected, declared, loops)
            })();
            ctx.rollback(scope);
            checked?;
        }
        Ok(())
    }
}

/// A value is executable unless its type is ghost, as a function returning
/// only a proof has.
fn expect(
    ctx: &mut Context,
    value: &Term,
    expected: &Type,
    definitions: &Definitions,
) -> Result<(), ExecError> {
    let mode = if definitions.is_erased_type(expected) {
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

/// Native signatures cannot contain erased fields, functions or invariant-
/// carrying nominal types. Their Rust identity is checked at the import edge.
fn foreign_type(ty: &Type) -> bool {
    ty.as_machine().is_some()
        || matches!(ty, Type::Bool)
        || matches!(ty, Type::Tuple(fields) if fields.iter().all(foreign_type))
}
