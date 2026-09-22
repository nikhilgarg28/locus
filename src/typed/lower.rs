//! `lower`: the typed tree to what is checked. Trusted.
//!
//! It is a desugaring by recursion on the tree, against one evaluation
//! order, left to right. A pure expression becomes a kernel term. An
//! expression that may not return, because it calls an ordinary function,
//! loops, or transfers control, is put in let-normal form: each such step
//! becomes a statement of the check IR, in source order, under the identity
//! the typed tree already gave it. Nothing is invented that a proof could
//! need to mention, so proofs written against the tree stay valid.
//!
//! An operator at a machine type, `+`, `-`, `*`, `/`, `%`, or unary minus,
//! is never read as a term, whatever its operands: it may panic, so it is
//! a statement of the check IR, `Stmt::Operate`, emitted in evaluation
//! order after its operands with the evidence and the learned identities
//! the tree carries, and its value is the result the tree named. Only the
//! wrapping methods, and the same operators on `Int`, are terms.
//!
//! Assignment has no form in the check IR. A binding declared `let mut` has
//! versions: the binding itself, and one more for each assignment, a `let`
//! of the old version with the assigned path replaced (`rebuilt`). Lowering
//! keeps the current version of every mutable binding (`Versions`) and
//! requires every mention the tree makes in an executable position to be
//! the current one; a stale mention is rejected, never silently kept, so
//! that the check IR reads what the erased program reads. A branch some arm
//! of which assigns a binding declared outside it becomes a match whose
//! result is a tuple: the new versions of the bindings assigned in any arm,
//! in declaration order, then the value (`join_type`). Each arm ends by
//! building that tuple from its own current versions, so an arm that does
//! not assign a binding passes the entry version through, and an arm that
//! transfers control contributes nothing. After the match the versions and
//! the value are bound by projection. The set of assigned bindings is
//! computed here and not taken from the tree; the tree supplies the
//! identities of the new versions, which its proofs may mention, and a tree
//! whose set differs is rejected. Tracked evidence (`let mut ok: @P`) is
//! M4's; here a proposition or a type that mentions a mutable binding names
//! the version current where it was written, which is what a snapshot means.

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::erased::{self, Module};
use crate::exec::{self, Arm, ExecError, ExecFn, ExecFnId, ForStmt, OperateStmt, Program};
use crate::kernel::derive::symm_at;
use crate::kernel::{
    CmpOp, Definitions, EnumId, FnId, HypId, KernelError, MachineInt, Op, Panic, Proof, PropId,
    PropVariant, StructId, Term, Type, VarId, same, same_type,
};

use super::tree::{
    Binder, Block, CompareOp, EnumItem, Expr, FnItem, Joined, MatchArm, Pattern, Step, Stmt,
    StructItem,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LowerError {
    Kernel(KernelError),
    Exec(ExecError),
    /// A math function's body must be pure: no ordinary calls, no `loop`, no
    /// control transfer.
    ImpureInMath(String),
    /// `break` or `continue` somewhere other than the end of a block.
    ControlInExpression,
    /// The body of a bounded `for` must end in `continue`.
    ForBodyMustContinue,
    /// An assignment to a binding that no `let mut` of the function
    /// declared: unknown to lowering, so it has no version to replace.
    AssignToUnknown(String),
    /// A mention of a version of a mutable binding that is not the current
    /// one. The tree names versions; lowering decides which is current.
    StaleMention(String),
    /// The versions an `if` or `match` says it joins are not the bindings
    /// its arms assign, in declaration order.
    JoinMismatch,
    /// An assignment inside the body of the state-passing `loop` or `for`
    /// to a binding declared outside it. M3 replaces those forms.
    AssignInLoop(String),
    /// A place's path steps into something that is not a product with that
    /// field.
    BadPlace(String),
    /// An assignment where a term is wanted: in a `math fn`, or in a
    /// branch a proof stands in.
    AssignmentInTerm,
}

impl From<KernelError> for LowerError {
    fn from(error: KernelError) -> Self {
        Self::Kernel(error)
    }
}

impl From<ExecError> for LowerError {
    fn from(error: ExecError) -> Self {
        Self::Exec(error)
    }
}

impl fmt::Display for LowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kernel(error) => write!(f, "{error}"),
            Self::Exec(error) => write!(f, "{error}"),
            Self::ImpureInMath(name) => {
                write!(f, "math fn {name} has a body that is not pure")
            }
            Self::ControlInExpression => f.write_str("break and continue may only end a block"),
            Self::ForBodyMustContinue => f.write_str("the body of a for must end in continue"),
            Self::AssignToUnknown(name) => {
                write!(f, "assignment to `{name}`, which no `let mut` declared")
            }
            Self::StaleMention(name) => write!(
                f,
                "`{name}` is mentioned at a version that is not its current one"
            ),
            Self::JoinMismatch => f.write_str(
                "the versions joined after a branch are not the bindings its arms assign",
            ),
            Self::AssignInLoop(name) => write!(
                f,
                "assignment to `{name}` inside a loop body, which was declared outside it"
            ),
            Self::BadPlace(name) => write!(f, "`{name}` has no such field to assign"),
            Self::AssignmentInTerm => f.write_str("an assignment where a term is wanted"),
        }
    }
}

impl std::error::Error for LowerError {}

/// What a declared function became.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FnRef {
    Math(FnId),
    Exec(ExecFnId),
}

/// Lowers a program item by item. Each item is checked as it is declared,
/// and the identity it receives is what later items use to refer to it.
#[derive(Clone, Debug)]
pub struct Session {
    program: Program,
    erased: Module,
}

impl Session {
    pub fn new(definitions: Definitions) -> Self {
        Self {
            program: Program::new(definitions),
            erased: Module::default(),
        }
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    /// The erasure of every item accepted so far. An item is erased only
    /// after its lowering has been checked, so nothing unverified is here.
    pub fn erased(&self) -> &Module {
        &self.erased
    }

    pub fn declare_struct(&mut self, item: &StructItem) -> Result<StructId, LowerError> {
        let fields = telescope(&item.fields);
        let id = self.program.definitions_mut().declare_struct(&fields)?;
        self.erased.structs.push(erased::erase_struct(id, item));
        Ok(id)
    }

    pub fn declare_enum(&mut self, item: &EnumItem) -> Result<EnumId, LowerError> {
        let variants: Vec<Type> = item
            .variants
            .iter()
            .map(|variant| telescope(&variant.payload))
            .collect();
        let id = self.program.definitions_mut().declare_enum(&variants)?;
        self.erased.enums.push(erased::erase_enum(id, item));
        Ok(id)
    }

    /// A declared proposition is wholly logical: the kernel checks it and
    /// nothing is emitted.
    pub fn declare_prop(
        &mut self,
        params: Vec<Type>,
        variants: Vec<PropVariant>,
    ) -> Result<PropId, LowerError> {
        Ok(self
            .program
            .definitions_mut()
            .declare_prop(params, variants)?)
    }

    pub fn declare_fn(&mut self, item: &FnItem) -> Result<FnRef, LowerError> {
        self.declare_fn_promising(item, exec::Promises::default())
    }

    /// `declare_fn` for a function that makes promises: the checker enforces
    /// each one on the check IR of an ordinary function. A math function is
    /// a kernel function, total and without effects by construction, so
    /// nothing of its promises is recorded.
    pub fn declare_fn_promising(
        &mut self,
        item: &FnItem,
        promises: exec::Promises,
    ) -> Result<FnRef, LowerError> {
        let reference = self.check_fn(item, promises)?;
        let erased = erased::erase_fn(self.program.definitions(), reference, item);
        self.erased.fns.extend(erased);
        Ok(reference)
    }

    /// A constant: a function of no parameters in the logic and the
    /// checker, and a `const` item in Rust, whose value Rust computes
    /// itself (the elaborator has seen that it can).
    pub fn declare_constant(
        &mut self,
        item: &FnItem,
        promises: exec::Promises,
    ) -> Result<FnRef, LowerError> {
        let reference = self.declare_fn_promising(item, promises)?;
        if let Some(function) = self.erased.fns.last_mut()
            && function.reference == reference
        {
            function.constant = true;
        }
        Ok(reference)
    }

    fn check_fn(&mut self, item: &FnItem, promises: exec::Promises) -> Result<FnRef, LowerError> {
        let params: Vec<(VarId, Type)> = item
            .params
            .iter()
            .map(|param| (param.id, param.ty.clone()))
            .collect();
        let signature = Type::function_over(&params, &item.result);
        if item.math {
            if !block_is_pure(&item.body) {
                return Err(LowerError::ImpureInMath(item.name.clone()));
            }
            let body = pure_block(&item.body)?;
            // The kernel names the parameters itself; hand the body over in
            // terms of its names.
            let id = self
                .program
                .definitions_mut()
                .declare_fn(&signature, |given| {
                    item.params
                        .iter()
                        .zip(given)
                        .fold(body, |body, (param, term)| body.replace_var(param.id, term))
                })?;
            Ok(FnRef::Math(id))
        } else {
            let function = ExecFn {
                promises,
                signature,
                params: item.params.iter().map(|param| param.id).collect(),
                body: lower_block(&item.body, &mut Versions::default(), End::Value)?,
            };
            Ok(FnRef::Exec(self.program.declare(function)?))
        }
    }
}

fn telescope(binders: &[Binder]) -> Type {
    let fields: Vec<(VarId, Type)> = binders
        .iter()
        .map(|binder| (binder.id, binder.ty.clone()))
        .collect();
    Type::tuple_over(&fields)
}

// --- Purity ---------------------------------------------------------------------

/// Whether evaluating the expression always returns and transfers no
/// control, so that it can be a kernel term.
pub fn is_pure(expr: &Expr) -> bool {
    match expr {
        Expr::Var { .. }
        | Expr::Bool(_)
        | Expr::Literal(..)
        | Expr::Int(_)
        | Expr::Proof(_)
        | Expr::Prop(_)
        | Expr::Absurd { .. } => true,
        // An operator at a machine type may panic, so it is a statement and
        // never a term, whatever its operands; only the wrapping methods
        // and the operators of `Int` are terms.
        Expr::CallFn { .. }
        | Expr::Loop { .. }
        | Expr::Break(_)
        | Expr::Continue(_)
        | Expr::Operate { .. } => false,
        Expr::IntArith { operands, .. } => operands.iter().all(is_pure),
        Expr::Tuple { fields, .. } => fields.iter().all(is_pure),
        Expr::Struct { fields, .. } => fields.iter().all(|(_, field)| is_pure(field)),
        Expr::Variant { payload, .. } => payload.iter().all(is_pure),
        Expr::Field { target, .. } => is_pure(target),
        Expr::Method {
            receiver,
            arguments,
            ..
        } => is_pure(receiver) && arguments.iter().all(is_pure),
        Expr::Compare { left, right, .. } => is_pure(left) && is_pure(right),
        Expr::Cast { expr, .. } => is_pure(expr),
        Expr::CallMath { arguments, .. } => arguments.iter().all(is_pure),
        Expr::If {
            condition,
            then_block,
            else_block,
            ..
        } => is_pure(condition) && block_is_pure(then_block) && block_is_pure(else_block),
        Expr::Match {
            scrutinee, arms, ..
        } => is_pure(scrutinee) && arms.iter().all(|arm| block_is_pure(&arm.body)),
        Expr::Block(block) => block_is_pure(block),
        // A for is pure when its body is, apart from the continue that ends it.
        Expr::For {
            lo,
            hi,
            state,
            body,
            ..
        } => {
            is_pure(lo)
                && is_pure(hi)
                && state.iter().all(|(_, init)| is_pure(init))
                && body.stmts.iter().all(stmt_is_pure)
                && matches!(body.tail.as_deref(), Some(Expr::Continue(next)) if next.iter().all(is_pure))
        }
    }
}

fn stmt_is_pure(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Expr(value) => is_pure(value),
        Stmt::Assign { .. } => false,
    }
}

fn block_is_pure(block: &Block) -> bool {
    block.stmts.iter().all(stmt_is_pure) && block.tail.as_deref().is_none_or(is_pure)
}

// --- Pure expressions: kernel terms ------------------------------------------------

fn unit() -> Term {
    Term::Tuple(Vec::new(), Vec::new())
}

fn pure_all(exprs: &[Expr]) -> Result<Vec<Term>, LowerError> {
    exprs.iter().map(pure).collect()
}

/// `if test { if_true } else { if_false }` as a term of type `bool`.
fn choose(test: Term, if_false: Term, if_true: Term) -> Term {
    Term::case_with(
        test,
        Type::Bool,
        vec![
            (Vec::new(), HypId::fresh(), if_false),
            (Vec::new(), HypId::fresh(), if_true),
        ],
    )
}

/// A comparison of two values of `ty`, as the kernel term whose value it
/// is. At a machine type, `==`, `<`, and `<=` are the primitives `eq[T]`,
/// `lt[T]`, and `le[T]`; `>` and `>=` are the last two with their operands
/// exchanged; `!=` is a case on `eq[T]` with the branches exchanged. At
/// `bool`, `==` is a case on the left operand, `if a { b } else { !b }`,
/// and `!=` the same with the branches exchanged; `bool` has no ordering.
fn compare(op: CompareOp, ty: &Type, left: Term, right: Term) -> Result<Term, LowerError> {
    let Some(machine) = ty.as_machine() else {
        if !matches!(ty, Type::Bool) || !matches!(op, CompareOp::Eq | CompareOp::Ne) {
            return Err(LowerError::Kernel(KernelError::TypeMismatch {
                expected: Type::U8,
                found: ty.clone(),
            }));
        }
        let not_right = choose(right.clone(), Term::Bool(true), Term::Bool(false));
        return Ok(match op {
            CompareOp::Eq => choose(left, not_right, right),
            _ => choose(left, right, not_right),
        });
    };
    let cmp = |op, a, b| Term::cmp(op, machine, a, b);
    Ok(match op {
        CompareOp::Eq => cmp(CmpOp::Eq, left, right),
        CompareOp::Lt => cmp(CmpOp::Lt, left, right),
        CompareOp::Le => cmp(CmpOp::Le, left, right),
        CompareOp::Gt => cmp(CmpOp::Lt, right, left),
        CompareOp::Ge => cmp(CmpOp::Le, right, left),
        // `a != b` as a value is the negation of the comparison.
        CompareOp::Ne => choose(
            cmp(CmpOp::Eq, left, right),
            Term::Bool(true),
            Term::Bool(false),
        ),
    })
}

/// `expr as to` for a value of type `from`: `cast[S, T]` between machine
/// types, `view[S]` into `Int`, and `wrap[T]` out of it.
fn cast(from: &Type, to: &Type, value: Term) -> Result<Term, LowerError> {
    Ok(match (from.as_machine(), to.as_machine()) {
        (Some(from), Some(to)) => Term::cast(from, to, value),
        (Some(from), None) if matches!(to, Type::Int) => Term::view(from, value),
        (None, Some(to)) if matches!(from, Type::Int) => Term::wrap(to, value),
        _ => {
            return Err(LowerError::Kernel(KernelError::TypeMismatch {
                expected: to.clone(),
                found: from.clone(),
            }));
        }
    })
}

/// The machine type of a `for` index; a binder of any other type is a
/// tree the elaborator never builds.
fn index_type(index: &Binder) -> Result<MachineInt, LowerError> {
    index
        .ty
        .as_machine()
        .ok_or(LowerError::Kernel(KernelError::TypeMismatch {
            expected: Type::U8,
            found: index.ty.clone(),
        }))
}

/// The comparison a condition performs, and whether the condition is its
/// negation. Branch facts are about this comparison, which is what the
/// kernel's reflection axioms speak of: `if a != b` branches on `a == b`
/// with the branches exchanged.
fn condition(expr: &Expr) -> (Expr, bool) {
    match expr {
        Expr::Compare {
            op: CompareOp::Ne,
            ty,
            left,
            right,
        } => (
            Expr::Compare {
                op: CompareOp::Eq,
                ty: ty.clone(),
                left: left.clone(),
                right: right.clone(),
            },
            true,
        ),
        other => (other.clone(), false),
    }
}

/// The `false` arm and then the `true` arm of an `if`, as (fact, block).
fn branches<'a>(
    negated: bool,
    then_fact: HypId,
    else_fact: HypId,
    then_block: &'a Block,
    else_block: &'a Block,
) -> [(HypId, &'a Block); 2] {
    if negated {
        [(then_fact, then_block), (else_fact, else_block)]
    } else {
        [(else_fact, else_block), (then_fact, then_block)]
    }
}

/// The kernel wants a proof-typed position inside a term to hold
/// `proof(...)`, which is what lets it ignore proofs when comparing terms. A
/// proof-typed variable, field, or call is a fine expression in the source,
/// so it is wrapped here.
fn canonical(expr: &Expr, term: Term) -> Term {
    if expr.is_proof() && !matches!(term, Term::Proof(_)) {
        Term::proof(Proof::OfTerm(term))
    } else {
        term
    }
}

fn pure(expr: &Expr) -> Result<Term, LowerError> {
    Ok(canonical(expr, pure_form(expr)?))
}

fn pure_form(expr: &Expr) -> Result<Term, LowerError> {
    Ok(match expr {
        Expr::Var { id, .. } => Term::var(*id),
        Expr::Bool(value) => Term::Bool(*value),
        Expr::Literal(ty, value) => Term::machine_int(*ty, *value),
        Expr::Int(value) => Term::Int(value.clone()),
        Expr::Tuple { ty, fields } => Term::tuple(ty, pure_all(fields)?),
        Expr::Struct { id, fields, .. } => Term::Struct(
            *id,
            fields
                .iter()
                .map(|(_, field)| pure(field))
                .collect::<Result<_, _>>()?,
        ),
        Expr::Variant {
            id, index, payload, ..
        } => Term::Variant(*id, *index, pure_all(payload)?),
        Expr::Field { target, index, .. } => Term::proj(pure(target)?, *index),
        Expr::Method {
            prim,
            receiver,
            arguments,
        } => {
            let mut operands = vec![pure(receiver)?];
            operands.extend(pure_all(arguments)?);
            Term::prim(*prim, operands)
        }
        Expr::Compare {
            op,
            ty,
            left,
            right,
        } => compare(*op, ty, pure(left)?, pure(right)?)?,
        Expr::Cast { expr, from, to } => cast(from, to, pure(expr)?)?,
        Expr::IntArith { op, operands } => int_arith(*op, pure_all(operands)?)?,
        Expr::CallMath { id, arguments, .. } => Term::call(Term::Fn(*id), pure_all(arguments)?),
        Expr::Proof(proof) => Term::proof(proof.clone()),
        Expr::Prop(prop) => prop.clone(),
        Expr::Absurd { proof, ty } => Term::Absurd(Box::new(proof.clone()), ty.clone()),
        Expr::If {
            condition: tested,
            then_fact,
            else_fact,
            then_block,
            else_block,
            ty,
            joined,
            ..
        } => {
            if joined.is_some() {
                return Err(LowerError::JoinMismatch);
            }
            let (comparison, negated) = condition(tested);
            let arms = branches(negated, *then_fact, *else_fact, then_block, else_block)
                .into_iter()
                .map(|(fact, block)| Ok((Vec::new(), fact, pure_block(block)?)))
                .collect::<Result<_, LowerError>>()?;
            Term::case_with(pure(&comparison)?, ty.clone(), arms)
        }
        Expr::Match {
            scrutinee,
            arms,
            ty,
            joined,
            ..
        } => {
            if joined.is_some() {
                return Err(LowerError::JoinMismatch);
            }
            let arms = arms
                .iter()
                .map(|arm| {
                    let payload = arm.payload.iter().map(|binder| binder.id).collect();
                    Ok((payload, arm.fact, pure_block(&arm.body)?))
                })
                .collect::<Result<_, LowerError>>()?;
            Term::case_with(pure(scrutinee)?, ty.clone(), arms)
        }
        Expr::Block(block) => pure_block(block)?,
        Expr::For {
            index,
            lower,
            upper,
            lo,
            hi,
            ordered,
            state,
            body,
            ..
        } => {
            let Some(Expr::Continue(next)) = body.tail.as_deref() else {
                return Err(LowerError::ForBodyMustContinue);
            };
            // The body's lets are substituted into the next state.
            let mut next = pure_all(next)?;
            for stmt in body.stmts.iter().rev() {
                next = next
                    .into_iter()
                    .map(|term| substitute_stmt(stmt, term))
                    .collect::<Result<_, _>>()?;
            }
            let binders: Vec<(VarId, Type)> = state
                .iter()
                .map(|(binder, _)| (binder.id, binder.ty.clone()))
                .collect();
            let init = state
                .iter()
                .map(|(_, init)| pure(init))
                .collect::<Result<_, _>>()?;
            Term::for_with(
                index_type(index)?,
                index.id,
                *lower,
                *upper,
                pure(lo)?,
                pure(hi)?,
                ordered.clone(),
                &binders,
                init,
                next,
            )
        }
        Expr::CallFn { .. }
        | Expr::Loop { .. }
        | Expr::Break(_)
        | Expr::Continue(_)
        | Expr::Operate { .. } => {
            return Err(LowerError::ControlInExpression);
        }
    })
}

/// `+`, `-`, `*`, `/`, `%`, or unary minus on `Int`: the primitive of the
/// same name, which is total. The operation must have its arity.
fn int_arith(op: Op, operands: Vec<Term>) -> Result<Term, LowerError> {
    if operands.len() != op.arity() || !matches!(op.panic(), Panic::Overflow | Panic::Division) {
        return Err(LowerError::Kernel(KernelError::WrongArity {
            expected: op.arity(),
            found: operands.len(),
        }));
    }
    Ok(op.exact_term(operands))
}

/// A pure block is its tail with its lets substituted in, last to first. A
/// kernel term has no `let`, and the equation a `let` would have provided
/// becomes an instance of reflexivity.
fn pure_block(block: &Block) -> Result<Term, LowerError> {
    let mut term = match block.tail.as_deref() {
        Some(tail) => pure(tail)?,
        None => unit(),
    };
    for stmt in block.stmts.iter().rev() {
        term = substitute_stmt(stmt, term)?;
    }
    Ok(term)
}

fn substitute_stmt(stmt: &Stmt, term: Term) -> Result<Term, LowerError> {
    match stmt {
        Stmt::Let { pattern, value } => substitute_pattern(pattern, &pure(value)?, term),
        // A pure expression statement has no effect.
        Stmt::Expr(_) => Ok(term),
        Stmt::Assign { .. } => Err(LowerError::AssignmentInTerm),
    }
}

/// The parts a pattern binds, in order, each with the value it is bound
/// to: a later part's value may mention an earlier name (`opened_part`).
fn bound_parts(pattern: &Pattern, value: &Term, earlier: &mut Vec<Named>) -> Vec<Named> {
    match pattern {
        Pattern::Wildcard => Vec::new(),
        Pattern::Bind {
            binder, equation, ..
        } => {
            let named = Named {
                id: binder.id,
                equation: *equation,
                ty: binder.ty.clone(),
                value: opened_part(value, &binder.ty, earlier),
            };
            if !matches!(binder.ty, Type::Proof(_)) {
                earlier.push(named.clone());
            }
            vec![named]
        }
        Pattern::Tuple(patterns) => patterns
            .iter()
            .enumerate()
            .flat_map(|(index, pattern)| {
                bound_parts(pattern, &Term::proj(value.clone(), index), earlier)
            })
            .collect(),
    }
}

/// Substitutes the names a pattern binds, last to first: a later part's
/// value may mention an earlier name, which the earlier substitution then
/// replaces.
fn substitute_pattern(pattern: &Pattern, value: &Term, term: Term) -> Result<Term, LowerError> {
    let parts = bound_parts(pattern, value, &mut Vec::new());
    Ok(parts.iter().rev().fold(term, |term, named| {
        term.replace_var(named.id, &named.value)
            .replace_hyp(named.equation, &Proof::Refl(named.value.clone()))
    }))
}

// --- Opening a dependent pattern ------------------------------------------------

/// A name an earlier part of a pattern bound, with the equation
/// `name == value` the checker has for it.
#[derive(Clone, Debug)]
pub struct Named {
    pub id: VarId,
    pub equation: HypId,
    pub ty: Type,
    pub value: Term,
}

/// Opening a dependent pattern: the value a part of a `let` pattern is bound
/// to, stated over the names the pattern bound before it.
///
/// In `let (next, still) = step(...)`, the second part is evidence whose
/// claim speaks of the first part as `step(...).0`. It is bound as
/// evidence of the claim over `next` instead: each earlier name's equation
/// `name == value` carries the evidence across, a fixed step the elaborator
/// and lowering both take, so that what the elaborator typed is what the
/// checker sees. A part that is not evidence, or whose claim mentions no
/// earlier name, is the projection itself.
///
/// `opened` is the part's type as the elaborator states it, over the
/// names; `part` is the projection the part stands for.
pub fn opened_part(part: &Term, opened: &Type, earlier: &[Named]) -> Term {
    let Type::Proof(claim) = opened else {
        return part.clone();
    };
    // The claim over the projections, which is what the kernel gives the
    // projection itself.
    let over_projections = earlier.iter().fold((**claim).clone(), |claim, named| {
        claim.replace_var(named.id, &named.value)
    });
    let (proof, _, changed) = open_claim(&over_projections, earlier, Proof::OfTerm(part.clone()));
    if changed {
        Term::proof(proof)
    } else {
        part.clone()
    }
}

/// The type of a part of a pattern, as the elaborator states it: a claim
/// over the projections restated over the names bound earlier.
pub fn opened_type(ty: &Type, earlier: &[Named]) -> Type {
    match ty {
        Type::Proof(claim) => {
            let (_, opened, _) = open_claim(claim, earlier, Proof::Omitted);
            Type::proof(opened)
        }
        other => other.clone(),
    }
}

/// Restates `claim` over the earlier names, one transport per name that
/// occurs, around `proof`: the proof, the claim it arrives at, and whether
/// any name occurred.
fn open_claim(claim: &Term, earlier: &[Named], proof: Proof) -> (Proof, Term, bool) {
    let mut current = claim.clone();
    let mut proof = proof;
    let mut changed = false;
    for named in earlier {
        if current.find(&|term| same(term, &named.value)).is_none() {
            continue;
        }
        let template = current.abstract_over(&|term| same(term, &named.value));
        current = template.open(&Term::var(named.id));
        proof = Proof::Transport {
            eq: Box::new(symm_at(
                &named.ty,
                &Term::var(named.id),
                Proof::hyp(named.equation),
            )),
            template,
            proof: Box::new(proof),
        };
        changed = true;
    }
    (proof, current, changed)
}

// --- Versions of mutable bindings ---------------------------------------------------

/// The current version of every binding declared `let mut` so far, and
/// which binding each version belongs to. An arm of a branch and the body
/// of a loop work on a copy, since what they assign does not reach the code
/// after them except through the join.
#[derive(Clone, Debug, Default)]
struct Versions {
    /// Binding to current version.
    current: HashMap<VarId, VarId>,
    /// Version to binding, for every version so far.
    binding: HashMap<VarId, VarId>,
    /// The declared name and type of each mutable binding.
    declared: HashMap<VarId, (String, Type)>,
    /// The mutable bindings in declaration order, which fixes the order of
    /// a join's tuple.
    order: Vec<VarId>,
}

impl Versions {
    /// `let mut`: the binding is its own first version.
    fn declare(&mut self, binder: &Binder) {
        self.current.insert(binder.id, binder.id);
        self.binding.insert(binder.id, binder.id);
        self.declared
            .insert(binder.id, (binder.name.clone(), binder.ty.clone()));
        self.order.push(binder.id);
    }

    fn current(&self, binding: VarId, name: &str) -> Result<VarId, LowerError> {
        self.current
            .get(&binding)
            .copied()
            .ok_or_else(|| LowerError::AssignToUnknown(name.to_string()))
    }

    fn declared_type(&self, binding: VarId, name: &str) -> Result<&Type, LowerError> {
        self.declared
            .get(&binding)
            .map(|(_, ty)| ty)
            .ok_or_else(|| LowerError::AssignToUnknown(name.to_string()))
    }

    /// A new version of a binding, which every later mention must use.
    fn assign(&mut self, binding: VarId, version: VarId, name: &str) -> Result<(), LowerError> {
        self.current(binding, name)?;
        self.current.insert(binding, version);
        self.binding.insert(version, binding);
        Ok(())
    }

    /// A mention in an executable position must be the current version of
    /// its binding, if it is a version of one at all.
    fn check_mention(&self, id: VarId, name: &str) -> Result<(), LowerError> {
        match self.binding.get(&id) {
            Some(binding) if self.current.get(binding) != Some(&id) => {
                Err(LowerError::StaleMention(name.to_string()))
            }
            _ => Ok(()),
        }
    }

    /// The bindings declared outside the given blocks and assigned inside
    /// any of them, nested blocks included, in declaration order. A write to
    /// a field counts for the root of its path, a binding declared inside
    /// is local to the block, and a binding that shadows another is a
    /// different binding, since identities are unique.
    fn assigned_in(&self, blocks: &[&Block]) -> Vec<VarId> {
        let mut roots = HashSet::new();
        let mut declared = HashSet::new();
        for block in blocks {
            visit_block(block, &mut |stmt| match stmt {
                Stmt::Assign { place, .. } => {
                    roots.insert(place.binding);
                }
                Stmt::Let { pattern, .. } => bound_ids(pattern, &mut declared),
                Stmt::Expr(_) => {}
            });
        }
        self.order
            .iter()
            .copied()
            .filter(|binding| roots.contains(binding) && !declared.contains(binding))
            .collect()
    }
}

fn bound_ids(pattern: &Pattern, out: &mut HashSet<VarId>) {
    match pattern {
        Pattern::Bind { binder, .. } => {
            out.insert(binder.id);
        }
        Pattern::Wildcard => {}
        Pattern::Tuple(patterns) => patterns.iter().for_each(|pattern| bound_ids(pattern, out)),
    }
}

/// Every statement under a block, in source order, nested blocks included.
pub(crate) fn visit_block(block: &Block, on_stmt: &mut dyn FnMut(&Stmt)) {
    for stmt in &block.stmts {
        on_stmt(stmt);
        match stmt {
            Stmt::Let { value, .. } | Stmt::Expr(value) | Stmt::Assign { value, .. } => {
                visit_expr(value, on_stmt);
            }
        }
    }
    if let Some(tail) = block.tail.as_deref() {
        visit_expr(tail, on_stmt);
    }
}

/// Every statement under an expression, in source order.
fn visit_expr(expr: &Expr, on_stmt: &mut dyn FnMut(&Stmt)) {
    each_expr(expr, &mut |expr| {
        let blocks: Vec<&Block> = match expr {
            Expr::If {
                then_block,
                else_block,
                ..
            } => vec![then_block, else_block],
            Expr::Match { arms, .. } => arms.iter().map(|arm| &arm.body).collect(),
            Expr::Block(block) | Expr::Loop { body: block, .. } | Expr::For { body: block, .. } => {
                vec![block]
            }
            _ => Vec::new(),
        };
        for block in blocks {
            for stmt in &block.stmts {
                on_stmt(stmt);
            }
        }
    });
}

/// Every expression under `expr`, itself included, in source order: the
/// statements' values and the tails of nested blocks among them.
fn each_expr(expr: &Expr, on_expr: &mut dyn FnMut(&Expr)) {
    on_expr(expr);
    let all = |exprs: &[Expr], on_expr: &mut dyn FnMut(&Expr)| {
        exprs.iter().for_each(|expr| each_expr(expr, on_expr));
    };
    let block = |block: &Block, on_expr: &mut dyn FnMut(&Expr)| {
        for stmt in &block.stmts {
            match stmt {
                Stmt::Let { value, .. } | Stmt::Expr(value) | Stmt::Assign { value, .. } => {
                    each_expr(value, on_expr);
                }
            }
        }
        if let Some(tail) = block.tail.as_deref() {
            each_expr(tail, on_expr);
        }
    };
    match expr {
        Expr::Var { .. }
        | Expr::Bool(_)
        | Expr::Literal(..)
        | Expr::Int(_)
        | Expr::Proof(_)
        | Expr::Prop(_)
        | Expr::Absurd { .. } => {}
        Expr::Tuple { fields, .. }
        | Expr::Variant {
            payload: fields, ..
        }
        | Expr::CallMath {
            arguments: fields, ..
        }
        | Expr::CallFn {
            arguments: fields, ..
        }
        | Expr::Continue(fields) => all(fields, on_expr),
        Expr::Struct { fields, .. } => fields
            .iter()
            .for_each(|(_, field)| each_expr(field, on_expr)),
        Expr::Field { target: inner, .. } | Expr::Cast { expr: inner, .. } | Expr::Break(inner) => {
            each_expr(inner, on_expr)
        }
        Expr::Method {
            receiver,
            arguments,
            ..
        } => {
            each_expr(receiver, on_expr);
            all(arguments, on_expr);
        }
        Expr::Operate { operands, .. } | Expr::IntArith { operands, .. } => all(operands, on_expr),
        Expr::Compare { left, right, .. } => {
            each_expr(left, on_expr);
            each_expr(right, on_expr);
        }
        Expr::If {
            condition,
            then_block,
            else_block,
            ..
        } => {
            each_expr(condition, on_expr);
            block(then_block, on_expr);
            block(else_block, on_expr);
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            each_expr(scrutinee, on_expr);
            arms.iter().for_each(|arm| block(&arm.body, on_expr));
        }
        Expr::Block(inner) => block(inner, on_expr),
        Expr::Loop { state, body, .. } => {
            state.iter().for_each(|(_, init)| each_expr(init, on_expr));
            block(body, on_expr);
        }
        Expr::For {
            lo,
            hi,
            state,
            body,
            ..
        } => {
            each_expr(lo, on_expr);
            each_expr(hi, on_expr);
            state.iter().for_each(|(_, init)| each_expr(init, on_expr));
            block(body, on_expr);
        }
    }
}

/// Every variable mentioned in an executable position of a pure expression
/// must be current. A pure expression contains no assignment, so all of its
/// mentions see the versions current where it stands.
fn mentions_current(expr: &Expr, env: &Versions) -> Result<(), LowerError> {
    let mut stale = None;
    each_expr(expr, &mut |expr| {
        if let Expr::Var { id, name, .. } = expr
            && stale.is_none()
            && let Err(error) = env.check_mention(*id, name)
        {
            stale = Some(error);
        }
    });
    stale.map_or(Ok(()), Err)
}

/// The value an assignment gives the whole binding: the current value with
/// the path replaced, rebuilt product by product from the inside out. A
/// field that holds evidence is carried over as `proof(...)`, which is how
/// the kernel wants a proof inside a term. Evidence about the assigned
/// field is carried over unchanged and is then evidence about the old
/// value, which the kernel rejects: such a field cannot be assigned alone.
/// The elaborator states the same term, so what it typed is what is checked.
pub fn rebuilt(current: Term, path: &[Step], value: Term) -> Result<Term, LowerError> {
    let Some((step, rest)) = path.split_first() else {
        return Ok(value);
    };
    let arity = step.proof_fields.len();
    let name = step.name.clone().unwrap_or_else(|| step.index.to_string());
    if step.index >= arity {
        return Err(LowerError::BadPlace(name));
    }
    let inner = rebuilt(Term::proj(current.clone(), step.index), rest, value)?;
    let fields = (0..arity)
        .map(|index| {
            if index == step.index {
                inner.clone()
            } else if step.proof_fields[index] {
                Term::proof(Proof::OfTerm(Term::proj(current.clone(), index)))
            } else {
                Term::proj(current.clone(), index)
            }
        })
        .collect();
    match &step.ty {
        Type::Tuple(types) if types.len() == arity => Ok(Term::tuple(&step.ty, fields)),
        Type::Struct(id) => Ok(Term::Struct(*id, fields)),
        _ => Err(LowerError::BadPlace(name)),
    }
}

/// The type of the tuple a branch with assignments produces: the joined
/// versions, each at its binding's declared type, then the value, whose
/// type may mention them.
pub fn join_type(joined: &Joined, result: VarId, value: &Type) -> Type {
    let mut fields: Vec<(VarId, Type)> = joined
        .joins
        .iter()
        .map(|join| (join.version.id, join.version.ty.clone()))
        .collect();
    fields.push((result, value.clone()));
    Type::tuple_over(&fields)
}

/// How a block ends: with its value, or, as an arm of a branch that joins
/// assigned bindings, with the tuple of their current versions and the
/// value.
#[derive(Clone, Copy)]
enum End<'a> {
    Value,
    Join { bindings: &'a [VarId], ty: &'a Type },
}

impl End<'_> {
    fn finish(self, value: Term, env: &Versions) -> Result<Term, LowerError> {
        match self {
            Self::Value => Ok(value),
            Self::Join { bindings, ty } => {
                let mut fields = bindings
                    .iter()
                    .map(|binding| {
                        let (name, _) = &env.declared[binding];
                        env.current(*binding, name).map(Term::var)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                fields.push(value);
                Ok(Term::tuple(ty, fields))
            }
        }
    }
}

// --- Expressions that may not return: the check IR --------------------------------

/// Lowers an expression, emitting a statement for each step that may not
/// return, in source order, and returns the pure term that stands for its
/// value. Every mention of a mutable binding must be its current version.
fn anf(expr: &Expr, out: &mut Vec<exec::Stmt>, env: &mut Versions) -> Result<Term, LowerError> {
    if is_pure(expr) {
        mentions_current(expr, env)?;
        return pure(expr);
    }
    Ok(canonical(expr, anf_form(expr, out, env)?))
}

fn anf_form(
    expr: &Expr,
    out: &mut Vec<exec::Stmt>,
    env: &mut Versions,
) -> Result<Term, LowerError> {
    let each = |exprs: &[Expr],
                out: &mut Vec<exec::Stmt>,
                env: &mut Versions|
     -> Result<Vec<Term>, LowerError> {
        exprs.iter().map(|expr| anf(expr, out, env)).collect()
    };
    Ok(match expr {
        Expr::Tuple { ty, fields } => Term::tuple(ty, each(fields, out, env)?),
        Expr::Struct { id, fields, .. } => {
            let values = fields
                .iter()
                .map(|(_, field)| anf(field, out, env))
                .collect::<Result<_, _>>()?;
            Term::Struct(*id, values)
        }
        Expr::Variant {
            id, index, payload, ..
        } => Term::Variant(*id, *index, each(payload, out, env)?),
        Expr::Field { target, index, .. } => Term::proj(anf(target, out, env)?, *index),
        Expr::Method {
            prim,
            receiver,
            arguments,
        } => {
            let mut operands = vec![anf(receiver, out, env)?];
            operands.extend(each(arguments, out, env)?);
            Term::prim(*prim, operands)
        }
        Expr::Compare {
            op,
            ty,
            left,
            right,
        } => {
            let left = anf(left, out, env)?;
            let right = anf(right, out, env)?;
            compare(*op, ty, left, right)?
        }
        Expr::Cast { expr, from, to } => cast(from, to, anf(expr, out, env)?)?,
        Expr::IntArith { op, operands } => int_arith(*op, each(operands, out, env)?)?,
        // The operands first, left to right, then the operation as a
        // statement; its value is the result the tree named.
        Expr::Operate {
            op,
            ty,
            operands,
            result,
            equation,
            fits,
            learned,
        } => {
            let arguments = each(operands, out, env)?;
            out.push(exec::Stmt::Operate(Box::new(OperateStmt {
                var: *result,
                equation: *equation,
                op: *op,
                ty: *ty,
                arguments,
                fits: fits.clone(),
                learned: learned.clone(),
            })));
            Term::var(*result)
        }
        Expr::CallMath { id, arguments, .. } => {
            Term::call(Term::Fn(*id), each(arguments, out, env)?)
        }
        Expr::CallFn {
            id,
            arguments,
            result,
            ..
        } => {
            let arguments = each(arguments, out, env)?;
            out.push(exec::Stmt::Call {
                var: *result,
                callee: *id,
                arguments,
            });
            Term::var(*result)
        }
        Expr::If {
            condition: tested,
            then_fact,
            else_fact,
            then_block,
            else_block,
            ty,
            result,
            joined,
        } => {
            let (comparison, negated) = condition(tested);
            // The condition is evaluated at the versions current on entry.
            let scrutinee = anf(&comparison, out, env)?;
            let arms = branches(negated, *then_fact, *else_fact, then_block, else_block)
                .into_iter()
                .map(|(fact, block)| (Vec::new(), fact, block))
                .collect();
            lower_branch(out, env, *result, ty, joined.as_ref(), scrutinee, arms)?
        }
        Expr::Match {
            scrutinee,
            arms,
            ty,
            result,
            joined,
            ..
        } => {
            let scrutinee = anf(scrutinee, out, env)?;
            let arms = arms
                .iter()
                .map(|arm| {
                    let payload = arm.payload.iter().map(|binder| binder.id).collect();
                    (payload, arm.fact, &arm.body)
                })
                .collect();
            lower_branch(out, env, *result, ty, joined.as_ref(), scrutinee, arms)?
        }
        Expr::Block(block) => {
            // Identities are unique, so splicing the block's statements into
            // the enclosing sequence cannot capture anything. What the block
            // assigns stays assigned, as in Rust.
            lower_stmts(&block.stmts, out, env)?;
            match block.tail.as_deref() {
                Some(tail) => anf(tail, out, env)?,
                None => unit(),
            }
        }
        Expr::Loop {
            state,
            result_ty,
            body,
            result,
        } => {
            let init = state
                .iter()
                .map(|(_, init)| anf(init, out, env))
                .collect::<Result<_, _>>()?;
            let binders: Vec<Binder> = state.iter().map(|(binder, _)| binder.clone()).collect();
            out.push(exec::Stmt::Loop {
                var: *result,
                state: telescope(&binders),
                vars: binders.iter().map(|binder| binder.id).collect(),
                init,
                result: result_ty.clone(),
                body: lower_loop_body(body, env)?,
            });
            Term::var(*result)
        }
        Expr::For {
            index,
            lower,
            upper,
            lo,
            hi,
            ordered,
            state,
            body,
            result,
        } => {
            let lo = anf(lo, out, env)?;
            let hi = anf(hi, out, env)?;
            let init = state
                .iter()
                .map(|(_, init)| anf(init, out, env))
                .collect::<Result<_, _>>()?;
            let binders: Vec<Binder> = state.iter().map(|(binder, _)| binder.clone()).collect();
            out.push(exec::Stmt::For(Box::new(ForStmt {
                var: *result,
                index: index.id,
                lower: *lower,
                upper: *upper,
                lo,
                hi,
                ordered: ordered.clone(),
                state: Type::function_over(&[(index.id, index.ty.clone())], &telescope(&binders)),
                vars: binders.iter().map(|binder| binder.id).collect(),
                init,
                body: lower_loop_body(body, env)?,
            })));
            Term::var(*result)
        }
        Expr::Break(_) | Expr::Continue(_) => return Err(LowerError::ControlInExpression),
        // Handled by the purity test above.
        Expr::Var { .. }
        | Expr::Bool(_)
        | Expr::Literal(..)
        | Expr::Int(_)
        | Expr::Proof(_)
        | Expr::Prop(_)
        | Expr::Absurd { .. } => pure(expr)?,
    })
}

/// The body of one of the state-passing loops. An assignment in it to a
/// binding declared outside it is refused until M3 replaces these forms:
/// the state of such a loop is what it passes, not what the body assigns.
fn lower_loop_body(body: &Block, env: &Versions) -> Result<exec::Block, LowerError> {
    if let Some(binding) = env.assigned_in(&[body]).first() {
        let (name, _) = &env.declared[binding];
        return Err(LowerError::AssignInLoop(name.clone()));
    }
    lower_block(body, &mut env.clone(), End::Value)
}

/// A branch in statement or value position: a match statement. When some
/// arm assigns a binding declared outside the branch, the match produces
/// the tuple of the assigned bindings' new versions and the value, each arm
/// ends by building it from its own versions, and afterwards the versions
/// and the value are bound by projection. The set of assigned bindings is
/// computed here; the tree's `joined` must name exactly those, in order.
fn lower_branch(
    out: &mut Vec<exec::Stmt>,
    env: &mut Versions,
    result: VarId,
    ty: &Type,
    joined: Option<&Joined>,
    scrutinee: Term,
    arms: Vec<(Vec<VarId>, HypId, &Block)>,
) -> Result<Term, LowerError> {
    let blocks: Vec<&Block> = arms.iter().map(|(_, _, block)| *block).collect();
    let assigned = env.assigned_in(&blocks);
    let lower_arms = |end: End<'_>, env: &Versions| -> Result<Vec<Arm>, LowerError> {
        arms.iter()
            .map(|(payload, fact, block)| {
                Ok(Arm {
                    payload: payload.clone(),
                    fact: *fact,
                    body: lower_block(block, &mut env.clone(), end)?,
                })
            })
            .collect()
    };
    if assigned.is_empty() {
        if joined.is_some() {
            return Err(LowerError::JoinMismatch);
        }
        let arms = lower_arms(End::Value, env)?;
        out.push(exec::Stmt::Match {
            var: result,
            ty: ty.clone(),
            scrutinee,
            arms,
        });
        return Ok(Term::var(result));
    }
    let joined = joined.ok_or(LowerError::JoinMismatch)?;
    let bindings: Vec<VarId> = joined.joins.iter().map(|join| join.binding).collect();
    if bindings != assigned {
        return Err(LowerError::JoinMismatch);
    }
    for join in &joined.joins {
        let declared = env.declared_type(join.binding, &join.version.name)?;
        if !same_type(declared, &join.version.ty) {
            return Err(LowerError::Kernel(KernelError::TypeMismatch {
                expected: declared.clone(),
                found: join.version.ty.clone(),
            }));
        }
    }
    let tuple = join_type(joined, result, ty);
    let arms = lower_arms(
        End::Join {
            bindings: &bindings,
            ty: &tuple,
        },
        env,
    )?;
    out.push(exec::Stmt::Match {
        var: joined.tuple,
        ty: tuple,
        scrutinee,
        arms,
    });
    // The versions and the value, opened as a `let` pattern opens a tuple:
    // evidence typed over a version is restated over the name bound to it.
    let mut parts: Vec<Pattern> = joined
        .joins
        .iter()
        .map(|join| Pattern::Bind {
            binder: join.version.clone(),
            equation: join.equation,
            mutable: false,
        })
        .collect();
    parts.push(Pattern::Bind {
        binder: Binder {
            id: result,
            name: String::new(),
            ty: ty.clone(),
        },
        equation: joined.equation,
        mutable: false,
    });
    bind_pattern(&Pattern::Tuple(parts), Term::var(joined.tuple), out, env)?;
    for join in &joined.joins {
        env.assign(join.binding, join.version.id, &join.version.name)?;
    }
    Ok(Term::var(result))
}

/// The term lowering uses for the expression's value, without lowering it:
/// what `anf` returns. The elaborator states goals with it, so the two must
/// agree; a disagreement shows up as a proof the kernel rejects, never as an
/// accepted program.
pub fn value_term(expr: &Expr) -> Result<Term, LowerError> {
    if is_pure(expr) {
        return pure(expr);
    }
    let each = |exprs: &[Expr]| -> Result<Vec<Term>, LowerError> {
        exprs.iter().map(value_term).collect()
    };
    let form = match expr {
        Expr::Tuple { ty, fields } => Term::tuple(ty, each(fields)?),
        Expr::Struct { id, fields, .. } => Term::Struct(
            *id,
            fields
                .iter()
                .map(|(_, field)| value_term(field))
                .collect::<Result<_, _>>()?,
        ),
        Expr::Variant {
            id, index, payload, ..
        } => Term::Variant(*id, *index, each(payload)?),
        Expr::Field { target, index, .. } => Term::proj(value_term(target)?, *index),
        Expr::Method {
            prim,
            receiver,
            arguments,
        } => {
            let mut operands = vec![value_term(receiver)?];
            operands.extend(each(arguments)?);
            Term::prim(*prim, operands)
        }
        Expr::Compare {
            op,
            ty,
            left,
            right,
        } => compare(*op, ty, value_term(left)?, value_term(right)?)?,
        Expr::Cast { expr, from, to } => cast(from, to, value_term(expr)?)?,
        Expr::IntArith { op, operands } => int_arith(*op, each(operands)?)?,
        Expr::CallMath { id, arguments, .. } => Term::call(Term::Fn(*id), each(arguments)?),
        // An operator at a machine type is never read as a term: its value
        // is the result of its statement.
        Expr::CallFn { result, .. }
        | Expr::Operate { result, .. }
        | Expr::If { result, .. }
        | Expr::Match { result, .. }
        | Expr::Loop { result, .. }
        | Expr::For { result, .. } => Term::var(*result),
        Expr::Block(block) => match block.tail.as_deref() {
            Some(tail) => return value_term(tail),
            None => unit(),
        },
        Expr::Break(_) | Expr::Continue(_) => return Err(LowerError::ControlInExpression),
        Expr::Var { .. }
        | Expr::Bool(_)
        | Expr::Literal(..)
        | Expr::Int(_)
        | Expr::Proof(_)
        | Expr::Prop(_)
        | Expr::Absurd { .. } => return pure(expr),
    };
    Ok(canonical(expr, form))
}

fn lower_arms(arms: &[MatchArm], env: &Versions, end: End<'_>) -> Result<Vec<Arm>, LowerError> {
    arms.iter()
        .map(|arm| {
            Ok(Arm {
                payload: arm.payload.iter().map(|binder| binder.id).collect(),
                fact: arm.fact,
                body: lower_block(&arm.body, &mut env.clone(), end)?,
            })
        })
        .collect()
}

fn lower_stmts(
    stmts: &[Stmt],
    out: &mut Vec<exec::Stmt>,
    env: &mut Versions,
) -> Result<(), LowerError> {
    for stmt in stmts {
        match stmt {
            Stmt::Let { pattern, value } => {
                let value = anf(value, out, env)?;
                bind_pattern(pattern, value, out, env)?;
            }
            Stmt::Expr(expr) => {
                anf(expr, out, env)?;
            }
            Stmt::Assign {
                place,
                value,
                version,
                equation,
            } => {
                // The right side first, with whatever it assigns; then the
                // place, from the versions current after it.
                let value = anf(value, out, env)?;
                let current = env.current(place.binding, &place.name)?;
                let ty = env.declared_type(place.binding, &place.name)?.clone();
                let value = rebuilt(Term::var(current), &place.path, value)?;
                out.push(exec::Stmt::Let {
                    var: version.id,
                    equation: *equation,
                    ty: Some(ty),
                    value,
                });
                env.assign(place.binding, version.id, &place.name)?;
            }
        }
    }
    Ok(())
}

fn bind_pattern(
    pattern: &Pattern,
    value: Term,
    out: &mut Vec<exec::Stmt>,
    env: &mut Versions,
) -> Result<(), LowerError> {
    for named in bound_parts(pattern, &value, &mut Vec::new()) {
        out.push(exec::Stmt::Let {
            var: named.id,
            equation: named.equation,
            ty: Some(named.ty),
            value: named.value,
        });
    }
    declare_mutable(pattern, env);
    Ok(())
}

/// The names a pattern binds with `mut` become mutable bindings, each its
/// own first version.
fn declare_mutable(pattern: &Pattern, env: &mut Versions) {
    match pattern {
        Pattern::Bind {
            binder,
            mutable: true,
            ..
        } => env.declare(binder),
        Pattern::Bind { .. } | Pattern::Wildcard => {}
        Pattern::Tuple(patterns) => patterns
            .iter()
            .for_each(|pattern| declare_mutable(pattern, env)),
    }
}

fn lower_block(block: &Block, env: &mut Versions, end: End<'_>) -> Result<exec::Block, LowerError> {
    let mut stmts = Vec::new();
    lower_stmts(&block.stmts, &mut stmts, env)?;
    let tail = match block.tail.as_deref() {
        None => exec::Tail::Value(end.finish(unit(), env)?),
        Some(tail) => lower_tail(tail, &mut stmts, env, end)?,
    };
    Ok(exec::Block { stmts, tail })
}

/// The end of a block: a control transfer, a match whose arms are blocks of
/// the same kind, or a value. A branch in tail position needs no join of
/// its own, whatever the tree recorded for it: nothing follows it but the
/// end of the enclosing block, and each arm reaches that end with its own
/// versions, which is what `end` builds from.
fn lower_tail(
    tail: &Expr,
    stmts: &mut Vec<exec::Stmt>,
    env: &mut Versions,
    end: End<'_>,
) -> Result<exec::Tail, LowerError> {
    Ok(match tail {
        Expr::Break(value) => exec::Tail::Break(anf(value, stmts, env)?),
        Expr::Continue(next) => exec::Tail::Continue(
            next.iter()
                .map(|expr| anf(expr, stmts, env))
                .collect::<Result<_, _>>()?,
        ),
        Expr::If {
            condition: tested,
            then_fact,
            else_fact,
            then_block,
            else_block,
            ..
        } if !is_pure(tail) => {
            let (comparison, negated) = condition(tested);
            let scrutinee = anf(&comparison, stmts, env)?;
            let arms = branches(negated, *then_fact, *else_fact, then_block, else_block)
                .into_iter()
                .map(|(fact, block)| {
                    Ok(Arm {
                        payload: Vec::new(),
                        fact,
                        body: lower_block(block, &mut env.clone(), end)?,
                    })
                })
                .collect::<Result<_, LowerError>>()?;
            exec::Tail::Match { scrutinee, arms }
        }
        Expr::Match {
            scrutinee, arms, ..
        } if !is_pure(tail) => {
            let scrutinee = anf(scrutinee, stmts, env)?;
            exec::Tail::Match {
                scrutinee,
                arms: lower_arms(arms, env, end)?,
            }
        }
        Expr::Block(block) if !is_pure(tail) => {
            lower_stmts(&block.stmts, stmts, env)?;
            match block.tail.as_deref() {
                Some(inner) => lower_tail(inner, stmts, env, end)?,
                None => exec::Tail::Value(end.finish(unit(), env)?),
            }
        }
        other => {
            let value = anf(other, stmts, env)?;
            exec::Tail::Value(end.finish(value, env)?)
        }
    })
}
