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
//! transfers control contributes nothing, and what it assigns does not put
//! a binding in the tuple, since nothing after the branch sees it
//! (`block_leaves`). After the match the versions and the value are bound
//! by projection. The set of assigned bindings is computed here and not
//! taken from the tree; the tree supplies the identities of the new
//! versions, which its proofs may mention, and a tree whose set differs is
//! rejected.
//!
//! Tracked evidence, `let mut ok: @P`, is a mutable binding like any other.
//! Its declared type mentions the versions of other mutable bindings that
//! were current where it was written, and the type of each of its versions,
//! at an assignment, in the tuple of a join, or in the state of a loop, is
//! that declared type over the versions current at that point
//! (`Versions::version_type`). Nothing else is needed: a refresh is an
//! assignment whose value is checked against that type, and a use of a
//! version whose type speaks of an old version of what it mentions, where
//! the claim over the current one is wanted, is a mismatch the checker
//! rejects. The elaborator's flow analysis, which says when evidence must be
//! refreshed, is not trusted for this; it gives the early error.
//!
//! A loop carries, as the state of the check IR's `loop` or `for`, the tuple
//! of the bindings declared outside it that its body assigns, in declaration
//! order, and for a `while` those its condition assigns too; the set is
//! computed here as a branch's is (`carried_bindings`), except that the
//! tree may leave out a binding of proof type, which the elaborator does
//! when the evidence is stale on entry; the versions the body makes of it
//! then stay in the body, and its version from before the loop remains a
//! fact about the versions it speaks of. The entry supplies
//! the versions current before the loop; the body sees the versions the
//! tree names for its state; each `continue`, and the end of the body,
//! supplies the versions current at that point; a `break` supplies them
//! too, after the value it carries when it is a `loop`'s. The loop's result
//! is the tuple of the versions after the loop, followed by the value for a
//! `loop`, and after the loop the versions and the value are bound by
//! projection as after a branch. `while c { body }` is a `loop` whose body
//! evaluates `c` and matches on it, `false` breaking and `true` running the
//! body to a `continue`. `for` is the check IR's bounded `for` with the same
//! state; nothing is asked about the order of its bounds, since an empty
//! range runs no pass. No loop lowers to a kernel term.
//!
//! A reference parameter is a value. `&T` adds nothing to the logic: the
//! parameter is the value lent. A `&mut T` parameter is a mutable binding
//! of the body, a value passed in and a new version passed out: the
//! function's result in the check IR is the tuple of the final versions of
//! its `&mut` parameters followed by its declared result (`exec_result`),
//! which the end of the body builds as the arm of a join does. A call with
//! `&mut` arguments binds that tuple, then gives the root of each lent
//! place a new version, a `let` of the old version with the path replaced
//! by the tuple's field, rebuilt as an assignment is, in argument order;
//! the call's value is the tuple's last field. Two arguments of one call
//! may not overlap when either is `&mut`, since two values written back to
//! one place would let the logic keep the wrong one: the check is here,
//! and trusted. What the interpreter of the check IR needs to report the
//! values of a function's `&mut` parameters at a panic is a side table
//! (`Lending`) the checker never reads.

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::erased::{self, Module};
use crate::exec::{self, Arm, ExecError, ExecFn, ExecFnId, ForStmt, Lending, OperateStmt, Program};
use crate::kernel::derive::symm_at;
use crate::kernel::{
    CmpOp, Definitions, EnumId, FnId, HypId, KernelError, Op, Panic, Proof, PropId, PropVariant,
    StructId, Term, Type, VarId, same, same_type,
};

use super::tree::{
    Binder, Block, Carried, CompareOp, EnumItem, Expr, FnItem, Join, Joined, Lend, MatchArm,
    PanicForm, Pattern, Step, Stmt, StructItem,
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
    /// `break` or `continue` with no loop around it.
    NoEnclosingLoop,
    /// `break` with a value in a `while` or a `for`, which produce none.
    BreakWithValue,
    /// An assignment to a binding that no `let mut` of the function
    /// declared: unknown to lowering, so it has no version to replace.
    AssignToUnknown(String),
    /// A mention of a version of a mutable binding that is not the current
    /// one. The tree names versions; lowering decides which is current.
    StaleMention(String),
    /// The versions an `if` or `match` says it joins, or a loop says it
    /// carries, are not the bindings its arms or its body assign, in
    /// declaration order.
    JoinMismatch,
    /// A place's path steps into something that is not a product with that
    /// field.
    BadPlace(String),
    /// An assignment where a term is wanted: in a `math fn`, or in a
    /// branch a proof stands in.
    AssignmentInTerm,
    /// Two arguments of one call name overlapping places, and one of them
    /// is lent by `&mut`: the write-backs would collide.
    OverlappingArguments(String),
    /// A `&mut` argument that is not a lent place, or a function whose
    /// exits do not match its `&mut` parameters.
    BadLend(String),
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
            Self::NoEnclosingLoop => f.write_str("break or continue outside a loop"),
            Self::BreakWithValue => {
                f.write_str("a break with a value in a while or a for, which produce none")
            }
            Self::AssignToUnknown(name) => {
                write!(f, "assignment to `{name}`, which no `let mut` declared")
            }
            Self::StaleMention(name) => write!(
                f,
                "`{name}` is mentioned at a version that is not its current one"
            ),
            Self::JoinMismatch => f.write_str(
                "the versions joined after a branch or a loop are not the bindings it assigns",
            ),
            Self::BadPlace(name) => write!(f, "`{name}` has no such field to assign"),
            Self::AssignmentInTerm => f.write_str("an assignment where a term is wanted"),
            Self::OverlappingArguments(name) => write!(
                f,
                "two arguments of one call overlap at `{name}`, and one is lent by `&mut`"
            ),
            Self::BadLend(name) => write!(f, "`{name}` is not a place lent to a call"),
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
    /// For each function with `&mut` parameters, what the interpreter of
    /// the check IR needs to report their values at a panic. The checker
    /// never reads it (`CheckInterpreter::with_lending`).
    lending: HashMap<ExecFnId, Lending>,
}

impl Session {
    pub fn new(definitions: Definitions) -> Self {
        Self {
            program: Program::new(definitions),
            erased: Module::default(),
            lending: HashMap::new(),
        }
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    /// The side table of every function with `&mut` parameters, for the
    /// interpreter of the check IR.
    pub fn lending(&self) -> &HashMap<ExecFnId, Lending> {
        &self.lending
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
        let erased = erased::erase_fn(&self.program, reference, item);
        self.erased.fns.extend(erased);
        Ok(reference)
    }

    /// `declare_fn_promising` for a function declared in an `impl` block
    /// (O4): `owner` is the type's name, and `receiver` says the first
    /// parameter is `self`. The logic and the checker see a function like
    /// any other, named `Type::name`; the erased tree records where it was
    /// declared so that the printer writes it inside `impl Type { .. }`.
    pub fn declare_method(
        &mut self,
        item: &FnItem,
        promises: exec::Promises,
        owner: &str,
        receiver: bool,
    ) -> Result<FnRef, LowerError> {
        let reference = self.declare_fn_promising(item, promises)?;
        if let Some(function) = self.erased.fns.last_mut()
            && function.reference == reference
        {
            function.owner = Some(owner.to_string());
            function.receiver = receiver;
        }
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
            let mut env = Versions::default();
            for (index, param) in item.params.iter().enumerate() {
                if item.passing_of(index).is_mutable() {
                    env.declare(param);
                }
            }
            let lent = item.lent_bindings();
            if lent.len() != item.exits.len() {
                return Err(LowerError::BadLend(item.name.clone()));
            }
            let result = item.exec_result();
            let signature = Type::function_over(&params, &result);
            // The end of the body supplies the tuple as the arm of a join
            // does, and so does every `return` (`function_result`).
            let body = if lent.is_empty() {
                lower_block(&item.body, &mut env, End::Value)?
            } else {
                env.result = Some((lent.clone(), result.clone()));
                lower_block(
                    &item.body,
                    &mut env,
                    End::Join {
                        bindings: &lent,
                        ty: &result,
                    },
                )?
            };
            let function = ExecFn {
                promises,
                signature,
                params: item.params.iter().map(|param| param.id).collect(),
                body,
            };
            let id = self.program.declare(function)?;
            if !lent.is_empty() {
                self.lending.insert(id, lending_of(item, &lent));
            }
            Ok(FnRef::Exec(id))
        }
    }
}

/// The value a `return` supplies, as the check IR wants it: for a function
/// with `&mut` parameters, the tuple of their current versions followed by
/// the value, as the end of the body supplies it; otherwise the value.
fn function_result(value: Term, env: &Versions) -> Result<Term, LowerError> {
    match &env.result {
        Some((bindings, ty)) => {
            let mut fields = env.versions(bindings)?;
            fields.push(value);
            Ok(Term::tuple(ty, fields))
        }
        None => Ok(value),
    }
}

/// What the interpreter of the check IR needs of a function with `&mut`
/// parameters: every version of each of them, so that the current value is
/// known at a panic, and for each call that lends a path into one of them,
/// where the callee's reported values go. Read from the tree, as erasure
/// reads the versions; the checker ignores it, so a mistake here is a
/// false alarm in the comparison of the interpreters and never a false
/// proof.
fn lending_of(item: &FnItem, lent: &[VarId]) -> Lending {
    let mut versions: HashMap<VarId, VarId> = HashMap::new();
    let mut calls = HashMap::new();
    let mut on_expr = |expr: &Expr| match expr {
        Expr::If { joined, .. } | Expr::Match { joined, .. } => {
            for join in joined.iter().flat_map(|joined| &joined.joins) {
                versions.insert(join.version.id, join.binding);
            }
        }
        Expr::Loop { state, carried, .. }
        | Expr::While { state, carried, .. }
        | Expr::For { state, carried, .. } => {
            for (inside, join) in state.iter().zip(&carried.joins) {
                versions.insert(inside.id, join.binding);
                versions.insert(join.version.id, join.binding);
            }
        }
        Expr::CallFn {
            arguments,
            result,
            lends,
            ..
        } => {
            let mut into_params = Vec::new();
            for (index, lend) in lends.iter().enumerate() {
                let Some(Expr::Lend { place, .. }) = arguments.get(lend.argument) else {
                    continue;
                };
                versions.insert(lend.version.id, place.binding);
                if let Some(position) = lent.iter().position(|binding| *binding == place.binding) {
                    let path = place.path.iter().map(|step| step.index).collect();
                    into_params.push((position, path, index));
                }
            }
            if !into_params.is_empty() {
                calls.insert(*result, into_params);
            }
        }
        _ => {}
    };
    each_expr_in_block(&item.body, &mut on_expr);
    let mut on_stmt = |stmt: &Stmt| {
        if let Stmt::Assign { place, version, .. } = stmt {
            versions.insert(version.id, place.binding);
        }
    };
    visit_block(&item.body, &mut on_stmt);
    let params = lent
        .iter()
        .map(|binding| {
            let index = item
                .params
                .iter()
                .position(|param| param.id == *binding)
                .expect("a lent binding is a parameter");
            let mut of_binding = vec![*binding];
            of_binding.extend(
                versions
                    .iter()
                    .filter(|(_, of)| *of == binding)
                    .map(|(version, _)| *version),
            );
            (index, of_binding)
        })
        .collect();
    Lending { params, calls }
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
/// control, so that it can be a kernel term. No loop is pure: a `for` over
/// a range is bounded, but it assigns or it does nothing, and either way
/// it is a statement of the check IR.
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
        | Expr::While { .. }
        | Expr::For { .. }
        | Expr::Break(_)
        | Expr::Continue
        | Expr::Operate { .. }
        | Expr::Panic { .. }
        | Expr::Return { .. }
        | Expr::Assert { .. } => false,
        Expr::IntArith { operands, .. } => operands.iter().all(is_pure),
        Expr::Lend { value, .. } => is_pure(value),
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
        Expr::Cast { expr, .. } | Expr::Ghost(expr) => is_pure(expr),
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
    }
}

fn stmt_is_pure(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Expr(value) => is_pure(value),
        Stmt::Assign { .. } => false,
    }
}

/// Whether every path through the block ends in `break`, `continue`,
/// `return`, or a panic: through the arms of an `if` or `match` in its
/// tail, and a block in its tail. Such an arm of a branch never reaches the
/// end of the branch, so lowering never builds the join's tuple from it
/// (`lower_tail`), and what it assigns is not the branch's to join. The
/// test is on the tree's shape, and says nothing of a loop that never
/// breaks.
pub fn block_leaves(block: &Block) -> bool {
    match block.tail.as_deref() {
        Some(Expr::Break(_) | Expr::Continue | Expr::Panic { .. } | Expr::Return { .. }) => true,
        Some(Expr::If {
            then_block,
            else_block,
            ..
        }) => block_leaves(then_block) && block_leaves(else_block),
        Some(Expr::Match { arms, .. }) => arms.iter().all(|arm| block_leaves(&arm.body)),
        Some(Expr::Block(inner)) => block_leaves(inner),
        _ => false,
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
        // A lent place is the value lent.
        Expr::Lend { value, .. } => pure(value)?,
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
        // A ghost value is its logical value; the kernel has no ghost
        // types, only ghost bindings.
        Expr::Ghost(expr) => pure(expr)?,
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
        Expr::CallFn { .. }
        | Expr::Loop { .. }
        | Expr::While { .. }
        | Expr::For { .. }
        | Expr::Break(_)
        | Expr::Continue
        | Expr::Operate { .. }
        | Expr::Panic { .. }
        | Expr::Return { .. }
        | Expr::Assert { .. } => {
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
/// after them except through the join. The loops around the position are
/// here too, for what `break` and `continue` must supply.
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
    /// The loops around the position, innermost last.
    loops: Vec<Frame>,
    /// For a function with `&mut` parameters: their bindings, in order, and
    /// the result type of the check IR, the tuple of their final versions
    /// followed by the value, which the end of the body and every `return`
    /// supply (`function_result`).
    result: Option<(Vec<VarId>, Type)>,
}

/// A loop around the position: the bindings it carries, the type of its
/// result, and whether a `break` carries a value after the versions, which
/// it does in a `loop` and not in a `while` or a `for`.
#[derive(Clone, Debug)]
struct Frame {
    bindings: Vec<VarId>,
    result: Type,
    valued: bool,
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

    /// The type a new version of a binding has here: its declared type,
    /// with every version of a mutable binding it mentions replaced by the
    /// current one. For a binding whose type mentions none, the declared
    /// type itself. This is what makes tracked evidence work: `ok: @P`
    /// declared over `lock` is, at each refresh, evidence of `P` over the
    /// `lock` of that moment, and a stale `ok` is a value of another type.
    fn version_type(&self, binding: VarId, name: &str) -> Result<Type, LowerError> {
        let mut ty = self.declared_type(binding, name)?.clone();
        for (version, of) in &self.binding {
            let current = self.current(*of, name)?;
            if *version != current {
                ty = ty.replace_var(*version, &Term::var(current));
            }
        }
        Ok(ty)
    }

    /// The types the tree must give the new versions of `bindings`, in
    /// order, each over the versions before it in the same list: the type
    /// of a join's tuple, or of a loop's state, field by field.
    fn version_types(&self, versions: &[(VarId, VarId, &str)]) -> Result<Vec<Type>, LowerError> {
        let mut env = self.clone();
        let mut types = Vec::new();
        for (binding, version, name) in versions {
            types.push(env.version_type(*binding, name)?);
            env.assign(*binding, *version, name)?;
        }
        Ok(types)
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

    /// The bindings declared outside the given blocks and expressions and
    /// assigned inside any of them, nested blocks included, in declaration
    /// order. A write to a field counts for the root of its path, a binding
    /// declared inside is local to the block, and a binding that shadows
    /// another is a different binding, since identities are unique.
    fn assigned_in(&self, blocks: &[&Block], exprs: &[&Expr]) -> Vec<VarId> {
        let mut roots = HashSet::new();
        let mut declared = HashSet::new();
        let mut on_stmt = |stmt: &Stmt| match stmt {
            Stmt::Assign { place, .. } => {
                roots.insert(place.binding);
            }
            Stmt::Let { pattern, .. } => bound_ids(pattern, &mut declared),
            Stmt::Expr(_) => {}
        };
        for block in blocks {
            visit_block(block, &mut on_stmt);
        }
        for expr in exprs {
            visit_expr(expr, &mut on_stmt);
        }
        // A place lent by `&mut` to a call is written back: a write to its
        // root, as a write to a field is.
        let mut on_expr = |expr: &Expr| {
            if let Expr::Lend {
                mutable: true,
                place,
                ..
            } = expr
            {
                roots.insert(place.binding);
            }
        };
        for block in blocks {
            each_expr_in_block(block, &mut on_expr);
        }
        for expr in exprs {
            each_expr(expr, &mut on_expr);
        }
        self.order
            .iter()
            .copied()
            .filter(|binding| roots.contains(binding) && !declared.contains(binding))
            .collect()
    }

    /// The current versions of the given bindings, as the fields of a
    /// tuple: a version of tracked evidence is given as the proof it is.
    fn versions(&self, bindings: &[VarId]) -> Result<Vec<Term>, LowerError> {
        bindings
            .iter()
            .map(|binding| {
                let (name, ty) = &self.declared[binding];
                let current = Term::var(self.current(*binding, name)?);
                Ok(if matches!(ty, Type::Proof(_)) {
                    Term::proof(Proof::OfTerm(current))
                } else {
                    current
                })
            })
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

/// Every expression under a block, in source order.
fn each_expr_in_block(block: &Block, on_expr: &mut dyn FnMut(&Expr)) {
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
pub(crate) fn visit_expr(expr: &Expr, on_stmt: &mut dyn FnMut(&Stmt)) {
    each_expr(expr, &mut |expr| {
        let blocks: Vec<&Block> = match expr {
            Expr::If {
                then_block,
                else_block,
                ..
            } => vec![then_block, else_block],
            Expr::Match { arms, .. } => arms.iter().map(|arm| &arm.body).collect(),
            Expr::Block(block)
            | Expr::Loop { body: block, .. }
            | Expr::While { body: block, .. }
            | Expr::For { body: block, .. } => vec![block],
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
pub(crate) fn each_expr(expr: &Expr, on_expr: &mut dyn FnMut(&Expr)) {
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
        } => all(fields, on_expr),
        Expr::Struct { fields, .. } => fields
            .iter()
            .for_each(|(_, field)| each_expr(field, on_expr)),
        Expr::Field { target: inner, .. }
        | Expr::Cast { expr: inner, .. }
        | Expr::Ghost(inner)
        | Expr::Lend { value: inner, .. } => each_expr(inner, on_expr),
        Expr::Break(value) | Expr::Return { value, .. } => {
            value.iter().for_each(|value| each_expr(value, on_expr));
        }
        Expr::Continue | Expr::Panic { .. } => {}
        Expr::Assert { condition, .. } => each_expr(condition, on_expr),
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
        Expr::Block(inner) | Expr::Loop { body: inner, .. } => block(inner, on_expr),
        Expr::While {
            condition, body, ..
        } => {
            each_expr(condition, on_expr);
            block(body, on_expr);
        }
        Expr::For { lo, hi, body, .. } => {
            each_expr(lo, on_expr);
            each_expr(hi, on_expr);
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

/// The type of the tuple a branch with assignments, or a loop, produces:
/// the joined versions, each at its binding's declared type, then the
/// value when there is one, whose type may mention them.
pub fn join_type(joins: &[Join], value: Option<(VarId, &Type)>) -> Type {
    let mut fields: Vec<(VarId, Type)> = joins
        .iter()
        .map(|join| (join.version.id, join.version.ty.clone()))
        .collect();
    fields.extend(value.map(|(result, ty)| (result, ty.clone())));
    Type::tuple_over(&fields)
}

/// How a block ends: with its value; as an arm of a branch that joins
/// assigned bindings, with the tuple of their current versions and the
/// value; or as the body of a loop, with `continue` at the current versions
/// of what the loop carries, its value dropped.
#[derive(Clone, Copy)]
enum End<'a> {
    Value,
    Join { bindings: &'a [VarId], ty: &'a Type },
    Continue,
}

impl End<'_> {
    fn finish(self, value: Term, env: &Versions) -> Result<exec::Tail, LowerError> {
        Ok(match self {
            Self::Value => exec::Tail::Value(value),
            Self::Join { bindings, ty } => {
                let mut fields = env.versions(bindings)?;
                fields.push(value);
                exec::Tail::Value(Term::tuple(ty, fields))
            }
            Self::Continue => {
                let frame = env.loops.last().ok_or(LowerError::NoEnclosingLoop)?;
                exec::Tail::Continue(env.versions(&frame.bindings)?)
            }
        })
    }
}

/// `break` from the innermost loop: its result tuple at the current
/// versions of what it carries, then the value in a `loop`.
fn breaking(value: Option<Term>, env: &Versions) -> Result<exec::Tail, LowerError> {
    let frame = env.loops.last().ok_or(LowerError::NoEnclosingLoop)?;
    let mut fields = env.versions(&frame.bindings)?;
    match (frame.valued, value) {
        (true, value) => fields.push(value.unwrap_or_else(unit)),
        (false, None) => {}
        (false, Some(_)) => return Err(LowerError::BreakWithValue),
    }
    Ok(exec::Tail::Break(Term::tuple(&frame.result, fields)))
}

/// The panic ending of one of the three forms, with the message Rust's
/// form of that name ends with.
fn panicking(form: PanicForm, argument: Option<&str>, unreachable: Option<Proof>) -> exec::Tail {
    exec::Tail::Panic {
        message: form.message(argument),
        unreachable,
    }
}

/// An ending that is not the end of its block, a panic or a `return`: a
/// match statement on `true` whose two arms both end in it. Every arm
/// leaves, so the checker declares `result`, the value the ending never
/// produces, at `ty`, the type expected of it, which is sound because
/// nothing after the statement is reached; the ending itself is checked in
/// each arm.
fn leaving(out: &mut Vec<exec::Stmt>, result: VarId, ty: &Type, tail: exec::Tail) -> Term {
    let arms = (0..2)
        .map(|_| Arm {
            payload: Vec::new(),
            fact: HypId::fresh(),
            body: exec::Block {
                stmts: Vec::new(),
                tail: tail.clone(),
            },
        })
        .collect();
    out.push(exec::Stmt::Match {
        var: result,
        ty: ty.clone(),
        scrutinee: Term::Bool(true),
        arms,
    });
    Term::var(result)
}

/// The bindings a loop carries: those declared outside it and assigned in
/// its body, or in its condition, in declaration order, which the tree's
/// `carried` and `state` must name exactly, at the bindings' declared types.
fn carried_bindings(
    env: &Versions,
    blocks: &[&Block],
    exprs: &[&Expr],
    state: &[Binder],
    carried: &Carried,
) -> Result<Vec<VarId>, LowerError> {
    let named: Vec<VarId> = carried.joins.iter().map(|join| join.binding).collect();
    // A binding of proof type may be left out: its version from before the
    // loop remains a true fact about the versions it speaks of, and the
    // versions the body makes of it stay in the body. A binding of data
    // left out would keep an equation the loop has falsified, so every
    // other assigned binding must be carried.
    let assigned: Vec<VarId> = env
        .assigned_in(blocks, exprs)
        .into_iter()
        .filter(|binding| {
            named.contains(binding) || !matches!(env.declared[binding].1, Type::Proof(_))
        })
        .collect();
    if named != assigned || state.len() != assigned.len() {
        return Err(LowerError::JoinMismatch);
    }
    // The versions the body sees, and those after the loop, each typed
    // over the ones before it.
    let inside: Vec<(VarId, VarId, &str)> = carried
        .joins
        .iter()
        .zip(state)
        .map(|(join, inside)| (join.binding, inside.id, join.version.name.as_str()))
        .collect();
    let after: Vec<(VarId, VarId, &str)> = carried
        .joins
        .iter()
        .map(|join| (join.binding, join.version.id, join.version.name.as_str()))
        .collect();
    let inside_types = env.version_types(&inside)?;
    let after_types = env.version_types(&after)?;
    for ((join, binder), (expected_inside, expected_after)) in carried
        .joins
        .iter()
        .zip(state)
        .zip(inside_types.iter().zip(&after_types))
    {
        for (expected, found) in [
            (expected_inside, &binder.ty),
            (expected_after, &join.version.ty),
        ] {
            if !same_type(expected, found) {
                return Err(LowerError::Kernel(KernelError::TypeMismatch {
                    expected: expected.clone(),
                    found: found.clone(),
                }));
            }
        }
    }
    Ok(assigned)
}

/// The versions a loop body works on: a copy of the versions at entry in
/// which each carried binding is at the version the tree names for its
/// state, under the loop's frame.
fn loop_env(
    env: &Versions,
    state: &[Binder],
    bindings: Vec<VarId>,
    result: Type,
    valued: bool,
) -> Result<Versions, LowerError> {
    let mut inner = env.clone();
    for (binder, binding) in state.iter().zip(&bindings) {
        inner.assign(*binding, binder.id, &binder.name)?;
    }
    inner.loops.push(Frame {
        bindings,
        result,
        valued,
    });
    Ok(inner)
}

/// After a branch or a loop: the versions and the value, opened from the
/// result tuple as a `let` pattern opens a tuple, so that evidence typed
/// over a version is restated over the name bound to it; then each joined
/// binding is at its new version.
fn bind_joined(
    out: &mut Vec<exec::Stmt>,
    env: &mut Versions,
    tuple: VarId,
    joins: &[Join],
    value: Option<(VarId, HypId, &Type)>,
) -> Result<(), LowerError> {
    let mut parts: Vec<Pattern> = joins
        .iter()
        .map(|join| Pattern::Bind {
            binder: join.version.clone(),
            equation: join.equation,
            mutable: false,
        })
        .collect();
    if let Some((result, equation, ty)) = value {
        parts.push(Pattern::Bind {
            binder: Binder {
                id: result,
                name: String::new(),
                ty: ty.clone(),
                ghost: false,
            },
            equation,
            mutable: false,
        });
    }
    bind_pattern(&Pattern::Tuple(parts), Term::var(tuple), out, env)?;
    for join in joins {
        env.assign(join.binding, join.version.id, &join.version.name)?;
    }
    Ok(())
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
        Expr::Ghost(inner) => anf(inner, out, env)?,
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
            lends,
            ..
        } => {
            disjoint_arguments(arguments, env)?;
            let terms = each(arguments, out, env)?;
            out.push(exec::Stmt::Call {
                var: *result,
                callee: *id,
                arguments: terms,
            });
            write_back(arguments, lends, *result, out, env)?;
            call_value(*result, lends)
        }
        // A lent place stands for its value; the call around it writes a
        // `&mut` one back.
        Expr::Lend { value, .. } => anf(value, out, env)?,
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
            carried,
            ty,
            result,
            equation,
            body,
        } => {
            let bindings = carried_bindings(env, &[body], &[], state, carried)?;
            let result_ty = join_type(&carried.joins, Some((*result, ty)));
            let init = env.versions(&bindings)?;
            let mut inner = loop_env(env, state, bindings, result_ty.clone(), true)?;
            let body = lower_block(body, &mut inner, End::Continue)?;
            out.push(exec::Stmt::Loop {
                var: carried.tuple,
                state: telescope(state),
                vars: state.iter().map(|binder| binder.id).collect(),
                init,
                result: result_ty,
                body,
            });
            bind_joined(
                out,
                env,
                carried.tuple,
                &carried.joins,
                Some((*result, *equation, ty)),
            )?;
            Term::var(*result)
        }
        Expr::While {
            condition: tested,
            then_fact,
            else_fact,
            state,
            carried,
            body,
        } => {
            let bindings = carried_bindings(env, &[body], &[tested], state, carried)?;
            // The result of a while is its state.
            let result_ty = telescope(state);
            let init = env.versions(&bindings)?;
            let mut inner = loop_env(env, state, bindings, result_ty.clone(), false)?;
            // Each pass evaluates the condition at the versions it starts
            // with, and leaves on `false` with the versions the condition
            // left, or runs the body on `true` and continues.
            let mut stmts = Vec::new();
            let (comparison, negated) = condition(tested);
            let scrutinee = anf(&comparison, &mut stmts, &mut inner)?;
            let exit = exec::Block {
                stmts: Vec::new(),
                tail: breaking(None, &inner)?,
            };
            let run = lower_block(body, &mut inner.clone(), End::Continue)?;
            let (if_false, if_true) = if negated {
                ((*then_fact, run), (*else_fact, exit))
            } else {
                ((*else_fact, exit), (*then_fact, run))
            };
            let arms = [if_false, if_true]
                .into_iter()
                .map(|(fact, body)| Arm {
                    payload: Vec::new(),
                    fact,
                    body,
                })
                .collect();
            out.push(exec::Stmt::Loop {
                var: carried.tuple,
                state: telescope(state),
                vars: state.iter().map(|binder| binder.id).collect(),
                init,
                result: result_ty,
                body: exec::Block {
                    stmts,
                    tail: exec::Tail::Match { scrutinee, arms },
                },
            });
            bind_joined(out, env, carried.tuple, &carried.joins, None)?;
            unit()
        }
        Expr::For {
            index,
            lower,
            upper,
            lo,
            hi,
            inclusive,
            state,
            carried,
            body,
        } => {
            let lo = anf(lo, out, env)?;
            let hi = anf(hi, out, env)?;
            let bindings = carried_bindings(env, &[body], &[], state, carried)?;
            // A for yields its state, on a break as at the end of the range.
            let result_ty = telescope(state);
            let init = env.versions(&bindings)?;
            let mut inner = loop_env(env, state, bindings, result_ty, false)?;
            let body = lower_block(body, &mut inner, End::Continue)?;
            out.push(exec::Stmt::For(Box::new(ForStmt {
                var: carried.tuple,
                index: index.id,
                lower: *lower,
                upper: *upper,
                lo,
                hi,
                inclusive: *inclusive,
                state: telescope(state),
                vars: state.iter().map(|binder| binder.id).collect(),
                init,
                body,
            })));
            bind_joined(out, env, carried.tuple, &carried.joins, None)?;
            unit()
        }
        Expr::Break(_) | Expr::Continue => return Err(LowerError::ControlInExpression),
        // A panic that is not the end of its block: a match on `true` whose
        // two arms both end in the panic. Every arm leaves, so the checker
        // declares the value the panic never produces, at the type expected
        // of it, and checks the evidence of `False` in each arm.
        Expr::Panic {
            form,
            argument,
            unreachable,
            ty,
            result,
        } => leaving(
            out,
            *result,
            ty,
            panicking(*form, argument.as_deref(), unreachable.clone()),
        ),
        // A `return` that is not the end of its block, the same way: the
        // value first, then a match on `true` whose arms both return it,
        // so that the checker checks it against the function's result type
        // in the context of this point.
        Expr::Return { value, ty, result } => {
            let value = match value {
                Some(value) => anf(value, out, env)?,
                None => unit(),
            };
            let value = function_result(value, env)?;
            leaving(out, *result, ty, exec::Tail::Return(value))
        }
        Expr::Assert {
            debug,
            condition: tested,
            then_fact,
            else_fact,
            message,
            unreachable,
            result,
        } => {
            let (comparison, negated) = condition(tested);
            let scrutinee = anf(&comparison, out, env)?;
            // The arm the check passes in yields the evidence that it did,
            // which is the fact of that arm; a `debug_assert!` yields `()`,
            // since a build may skip it. The other arm panics.
            let (ty, passed) = if *debug {
                (Type::Tuple(Vec::new()), unit())
            } else {
                let holds = Term::eq(Type::Bool, scrutinee.clone(), Term::Bool(!negated));
                (Type::proof(holds), Term::proof(Proof::hyp(*then_fact)))
            };
            let pass = exec::Block {
                stmts: Vec::new(),
                tail: exec::Tail::Value(passed),
            };
            let fail = exec::Block {
                stmts: Vec::new(),
                tail: exec::Tail::Panic {
                    message: message.clone(),
                    unreachable: unreachable.clone(),
                },
            };
            let (if_false, if_true) = if negated {
                ((*then_fact, pass), (*else_fact, fail))
            } else {
                ((*else_fact, fail), (*then_fact, pass))
            };
            let arms = [if_false, if_true]
                .into_iter()
                .map(|(fact, body)| Arm {
                    payload: Vec::new(),
                    fact,
                    body,
                })
                .collect();
            out.push(exec::Stmt::Match {
                var: *result,
                ty,
                scrutinee,
                arms,
            });
            unit()
        }
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

/// The value of a call: the result the tree named, or, for a call with
/// `&mut` arguments, the last field of the tuple the callee returns.
fn call_value(result: VarId, lends: &[Lend]) -> Term {
    if lends.is_empty() {
        Term::var(result)
    } else {
        Term::proj(Term::var(result), lends.len())
    }
}

/// The place an argument names, when it is a lend or a plain read of a
/// place rooted at a mutable binding: the binding, the path into it by
/// position, whether it is lent by `&mut`, and the root's name.
struct ArgumentPlace {
    root: VarId,
    path: Vec<usize>,
    mutable: bool,
    name: String,
}

fn argument_place(argument: &Expr, env: &Versions) -> Option<ArgumentPlace> {
    match argument {
        Expr::Lend { mutable, place, .. } => Some(ArgumentPlace {
            root: place.binding,
            path: place.path.iter().map(|step| step.index).collect(),
            mutable: *mutable,
            name: place.name.clone(),
        }),
        _ => {
            let mut path = Vec::new();
            let mut expr = argument;
            loop {
                match expr {
                    Expr::Field { target, index, .. } => {
                        path.push(*index);
                        expr = target;
                    }
                    Expr::Var { id, name, .. } => {
                        let root = *env.binding.get(id)?;
                        path.reverse();
                        return Some(ArgumentPlace {
                            root,
                            path,
                            mutable: false,
                            name: name.clone(),
                        });
                    }
                    _ => return None,
                }
            }
        }
    }
}

/// Two arguments of one call may not overlap, one path a prefix of the
/// other at the same root, when either is lent by `&mut`: the write-backs
/// would collide, and the logic could keep the wrong one. Trusted.
fn disjoint_arguments(arguments: &[Expr], env: &Versions) -> Result<(), LowerError> {
    let places: Vec<Option<ArgumentPlace>> = arguments
        .iter()
        .map(|argument| argument_place(argument, env))
        .collect();
    for (i, first) in places.iter().enumerate() {
        let Some(first) = first else {
            continue;
        };
        for second in places.iter().skip(i + 1).flatten() {
            let overlap = first.root == second.root
                && first.path.iter().zip(&second.path).all(|(a, b)| a == b);
            if overlap && (first.mutable || second.mutable) {
                return Err(LowerError::OverlappingArguments(first.name.clone()));
            }
        }
    }
    Ok(())
}

/// After a call with `&mut` arguments: each lent root gets a new version,
/// the old version with the lent path replaced by the value the callee
/// returned for it, in argument order.
fn write_back(
    arguments: &[Expr],
    lends: &[Lend],
    result: VarId,
    out: &mut Vec<exec::Stmt>,
    env: &mut Versions,
) -> Result<(), LowerError> {
    for (index, lend) in lends.iter().enumerate() {
        let Some(Expr::Lend {
            mutable: true,
            place,
            ..
        }) = arguments.get(lend.argument)
        else {
            return Err(LowerError::BadLend(lend.version.name.clone()));
        };
        let current = env.current(place.binding, &place.name)?;
        let ty = env.version_type(place.binding, &place.name)?;
        let returned = Term::proj(Term::var(result), index);
        let value = rebuilt(Term::var(current), &place.path, returned)?;
        out.push(exec::Stmt::Let {
            var: lend.version.id,
            equation: lend.equation,
            ty: Some(ty),
            value,
        });
        env.assign(place.binding, lend.version.id, &place.name)?;
    }
    Ok(())
}

/// A branch in statement or value position: a match statement. When some
/// arm that reaches the end of the branch assigns a binding declared
/// outside it, the match produces the tuple of the assigned bindings' new
/// versions and the value, each such arm ends by building it from its own
/// versions, and afterwards the versions and the value are bound by
/// projection. The set of assigned bindings is computed here, over the arms
/// that do not leave (`block_leaves`); the tree's `joined` must name
/// exactly those, in order.
fn lower_branch(
    out: &mut Vec<exec::Stmt>,
    env: &mut Versions,
    result: VarId,
    ty: &Type,
    joined: Option<&Joined>,
    scrutinee: Term,
    arms: Vec<(Vec<VarId>, HypId, &Block)>,
) -> Result<Term, LowerError> {
    // What the branch assigns is what the arms that reach its end assign:
    // an arm that leaves supplies no tuple, so nothing it assigned is seen
    // after the branch.
    let reaching: Vec<&Block> = arms
        .iter()
        .map(|(_, _, block)| *block)
        .filter(|block| !block_leaves(block))
        .collect();
    // As for a loop, a binding of proof type may be left out of the join:
    // its version from before the branch remains a true fact, and the
    // versions the arms make of it stay in the arms.
    let named: Vec<VarId> = joined
        .map(|joined| joined.joins.iter().map(|join| join.binding).collect())
        .unwrap_or_default();
    let assigned: Vec<VarId> = env
        .assigned_in(&reaching, &[])
        .into_iter()
        .filter(|binding| {
            named.contains(binding) || !matches!(env.declared[binding].1, Type::Proof(_))
        })
        .collect();
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
    let after: Vec<(VarId, VarId, &str)> = joined
        .joins
        .iter()
        .map(|join| (join.binding, join.version.id, join.version.name.as_str()))
        .collect();
    for (join, expected) in joined.joins.iter().zip(env.version_types(&after)?) {
        if !same_type(&expected, &join.version.ty) {
            return Err(LowerError::Kernel(KernelError::TypeMismatch {
                expected,
                found: join.version.ty.clone(),
            }));
        }
    }
    let tuple = join_type(&joined.joins, Some((result, ty)));
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
    bind_joined(
        out,
        env,
        joined.tuple,
        &joined.joins,
        Some((result, joined.equation, ty)),
    )?;
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
        Expr::Ghost(inner) => return value_term(inner),
        Expr::IntArith { op, operands } => int_arith(*op, each(operands)?)?,
        Expr::CallMath { id, arguments, .. } => Term::call(Term::Fn(*id), each(arguments)?),
        // An operator at a machine type is never read as a term: its value
        // is the result of its statement.
        Expr::CallFn { result, lends, .. } => call_value(*result, lends),
        Expr::Lend { value, .. } => return value_term(value),
        Expr::Operate { result, .. }
        | Expr::If { result, .. }
        | Expr::Match { result, .. }
        | Expr::Loop { result, .. } => Term::var(*result),
        Expr::While { .. } | Expr::For { .. } => unit(),
        Expr::Block(block) => match block.tail.as_deref() {
            Some(tail) => return value_term(tail),
            None => unit(),
        },
        Expr::Break(_) | Expr::Continue => return Err(LowerError::ControlInExpression),
        // A panic's or a return's value is the one its statement declares
        // and never produces; an assertion is a statement of type `()`.
        Expr::Panic { result, .. } | Expr::Return { result, .. } => Term::var(*result),
        Expr::Assert { .. } => unit(),
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
                let ty = env.version_type(place.binding, &place.name)?;
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
        None => end.finish(unit(), env)?,
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
        // The value first, with whatever it assigns; then the versions.
        Expr::Break(value) => {
            let value = match value {
                Some(value) => Some(anf(value, stmts, env)?),
                None => None,
            };
            breaking(value, env)?
        }
        Expr::Continue => End::Continue.finish(unit(), env)?,
        Expr::Panic {
            form,
            argument,
            unreachable,
            ..
        } => panicking(*form, argument.as_deref(), unreachable.clone()),
        // The function's ending, whatever block it stands in: the value
        // first, with whatever it assigns, at the versions current here.
        Expr::Return { value, .. } => {
            let value = match value {
                Some(value) => anf(value, stmts, env)?,
                None => unit(),
            };
            exec::Tail::Return(function_result(value, env)?)
        }
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
                None => end.finish(unit(), env)?,
            }
        }
        other => {
            let value = anf(other, stmts, env)?;
            end.finish(value, env)?
        }
    })
}
