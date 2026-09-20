//! `lower`: the typed tree to what is checked. Trusted.
//!
//! It is a desugaring by recursion on the tree, against one evaluation
//! order, left to right. A pure expression becomes a kernel term. An
//! expression that may not return, because it calls an ordinary function,
//! loops, or transfers control, is put in let-normal form: each such step
//! becomes a statement of the check IR, in source order, under the identity
//! the typed tree already gave it. Nothing is invented that a proof could
//! need to mention, so proofs written against the tree stay valid.

use std::fmt;

use crate::erased::{self, Module};
use crate::exec::{self, Arm, ExecError, ExecFn, ExecFnId, ForStmt, Program};
use crate::kernel::{
    Definitions, EnumId, FnId, HypId, KernelError, Prim, Proof, StructId, Term, Type, VarId,
};

use super::tree::{
    Binder, Block, CompareOp, EnumItem, Expr, FnItem, MatchArm, Pattern, Stmt, StructItem,
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

    pub fn declare_fn(&mut self, item: &FnItem) -> Result<FnRef, LowerError> {
        let reference = self.check_fn(item)?;
        let erased = erased::erase_fn(self.program.definitions(), reference, item);
        self.erased.fns.extend(erased);
        Ok(reference)
    }

    fn check_fn(&mut self, item: &FnItem) -> Result<FnRef, LowerError> {
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
                signature,
                params: item.params.iter().map(|param| param.id).collect(),
                body: lower_block(&item.body)?,
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
fn is_pure(expr: &Expr) -> bool {
    match expr {
        Expr::Var { .. }
        | Expr::Bool(_)
        | Expr::U8(_)
        | Expr::Proof(_)
        | Expr::Prop(_)
        | Expr::Absurd { .. } => true,
        Expr::CallFn { .. } | Expr::Loop { .. } | Expr::Break(_) | Expr::Continue(_) => false,
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

fn compare(op: CompareOp, left: Term, right: Term) -> Term {
    let prim = |prim, a, b| Term::prim(prim, vec![a, b]);
    match op {
        CompareOp::Eq => prim(Prim::U8Eq, left, right),
        CompareOp::Lt => prim(Prim::U8Lt, left, right),
        CompareOp::Le => prim(Prim::U8Le, left, right),
        CompareOp::Gt => prim(Prim::U8Lt, right, left),
        CompareOp::Ge => prim(Prim::U8Le, right, left),
        // `a != b` as a value is the negation of the comparison.
        CompareOp::Ne => Term::case_with(
            prim(Prim::U8Eq, left, right),
            Type::Bool,
            vec![
                (Vec::new(), HypId::fresh(), Term::Bool(true)),
                (Vec::new(), HypId::fresh(), Term::Bool(false)),
            ],
        ),
    }
}

/// The comparison a condition performs, and whether the condition is its
/// negation. Branch facts are about this comparison, which is what the
/// kernel's reflection axioms speak of: `if a != b` branches on `a == b`
/// with the branches exchanged.
fn condition(expr: &Expr) -> (Expr, bool) {
    match expr {
        Expr::Compare {
            op: CompareOp::Ne,
            left,
            right,
        } => (
            Expr::Compare {
                op: CompareOp::Eq,
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
        Expr::U8(value) => Term::U8(*value),
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
        Expr::Compare { op, left, right } => compare(*op, pure(left)?, pure(right)?),
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
            ..
        } => {
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
            ..
        } => {
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
        Expr::CallFn { .. } | Expr::Loop { .. } | Expr::Break(_) | Expr::Continue(_) => {
            return Err(LowerError::ControlInExpression);
        }
    })
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
    }
}

fn substitute_pattern(pattern: &Pattern, value: &Term, term: Term) -> Result<Term, LowerError> {
    Ok(match pattern {
        Pattern::Wildcard => term,
        Pattern::Bind { binder, equation } => term
            .replace_var(binder.id, value)
            .replace_hyp(*equation, &Proof::Refl(value.clone())),
        Pattern::Tuple(patterns) => {
            let mut term = term;
            for (index, pattern) in patterns.iter().enumerate() {
                term = substitute_pattern(pattern, &Term::proj(value.clone(), index), term)?;
            }
            term
        }
    })
}

// --- Expressions that may not return: the check IR --------------------------------

/// Lowers an expression, emitting a statement for each step that may not
/// return, in source order, and returns the pure term that stands for its
/// value.
fn anf(expr: &Expr, out: &mut Vec<exec::Stmt>) -> Result<Term, LowerError> {
    if is_pure(expr) {
        return pure(expr);
    }
    Ok(canonical(expr, anf_form(expr, out)?))
}

fn anf_form(expr: &Expr, out: &mut Vec<exec::Stmt>) -> Result<Term, LowerError> {
    let each = |exprs: &[Expr], out: &mut Vec<exec::Stmt>| -> Result<Vec<Term>, LowerError> {
        exprs.iter().map(|expr| anf(expr, out)).collect()
    };
    Ok(match expr {
        Expr::Tuple { ty, fields } => Term::tuple(ty, each(fields, out)?),
        Expr::Struct { id, fields, .. } => {
            let values = fields
                .iter()
                .map(|(_, field)| anf(field, out))
                .collect::<Result<_, _>>()?;
            Term::Struct(*id, values)
        }
        Expr::Variant {
            id, index, payload, ..
        } => Term::Variant(*id, *index, each(payload, out)?),
        Expr::Field { target, index, .. } => Term::proj(anf(target, out)?, *index),
        Expr::Method {
            prim,
            receiver,
            arguments,
        } => {
            let mut operands = vec![anf(receiver, out)?];
            operands.extend(each(arguments, out)?);
            Term::prim(*prim, operands)
        }
        Expr::Compare { op, left, right } => {
            let left = anf(left, out)?;
            let right = anf(right, out)?;
            compare(*op, left, right)
        }
        Expr::CallMath { id, arguments, .. } => Term::call(Term::Fn(*id), each(arguments, out)?),
        Expr::CallFn {
            id,
            arguments,
            result,
            ..
        } => {
            let arguments = each(arguments, out)?;
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
        } => {
            let (comparison, negated) = condition(tested);
            let scrutinee = anf(&comparison, out)?;
            let arms = branches(negated, *then_fact, *else_fact, then_block, else_block)
                .into_iter()
                .map(|(fact, block)| {
                    Ok(Arm {
                        payload: Vec::new(),
                        fact,
                        body: lower_block(block)?,
                    })
                })
                .collect::<Result<_, LowerError>>()?;
            out.push(exec::Stmt::Match {
                var: *result,
                ty: ty.clone(),
                scrutinee,
                arms,
            });
            Term::var(*result)
        }
        Expr::Match {
            scrutinee,
            arms,
            ty,
            result,
            ..
        } => {
            let scrutinee = anf(scrutinee, out)?;
            out.push(exec::Stmt::Match {
                var: *result,
                ty: ty.clone(),
                scrutinee,
                arms: lower_arms(arms)?,
            });
            Term::var(*result)
        }
        Expr::Block(block) => {
            // Identities are unique, so splicing the block's statements into
            // the enclosing sequence cannot capture anything.
            lower_stmts(&block.stmts, out)?;
            match block.tail.as_deref() {
                Some(tail) => anf(tail, out)?,
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
                .map(|(_, init)| anf(init, out))
                .collect::<Result<_, _>>()?;
            let binders: Vec<Binder> = state.iter().map(|(binder, _)| binder.clone()).collect();
            out.push(exec::Stmt::Loop {
                var: *result,
                state: telescope(&binders),
                vars: binders.iter().map(|binder| binder.id).collect(),
                init,
                result: result_ty.clone(),
                body: lower_block(body)?,
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
            let lo = anf(lo, out)?;
            let hi = anf(hi, out)?;
            let init = state
                .iter()
                .map(|(_, init)| anf(init, out))
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
                state: Type::function_over(&[(index.id, Type::U8)], &telescope(&binders)),
                vars: binders.iter().map(|binder| binder.id).collect(),
                init,
                body: lower_block(body)?,
            })));
            Term::var(*result)
        }
        Expr::Break(_) | Expr::Continue(_) => return Err(LowerError::ControlInExpression),
        // Handled by the purity test above.
        Expr::Var { .. }
        | Expr::Bool(_)
        | Expr::U8(_)
        | Expr::Proof(_)
        | Expr::Prop(_)
        | Expr::Absurd { .. } => pure(expr)?,
    })
}

fn lower_arms(arms: &[MatchArm]) -> Result<Vec<Arm>, LowerError> {
    arms.iter()
        .map(|arm| {
            Ok(Arm {
                payload: arm.payload.iter().map(|binder| binder.id).collect(),
                fact: arm.fact,
                body: lower_block(&arm.body)?,
            })
        })
        .collect()
}

fn lower_stmts(stmts: &[Stmt], out: &mut Vec<exec::Stmt>) -> Result<(), LowerError> {
    for stmt in stmts {
        match stmt {
            Stmt::Let { pattern, value } => {
                let value = anf(value, out)?;
                bind_pattern(pattern, value, out);
            }
            Stmt::Expr(expr) => {
                anf(expr, out)?;
            }
        }
    }
    Ok(())
}

fn bind_pattern(pattern: &Pattern, value: Term, out: &mut Vec<exec::Stmt>) {
    match pattern {
        Pattern::Wildcard => {}
        Pattern::Bind { binder, equation } => out.push(exec::Stmt::Let {
            var: binder.id,
            equation: *equation,
            ty: Some(binder.ty.clone()),
            value,
        }),
        Pattern::Tuple(patterns) => {
            for (index, pattern) in patterns.iter().enumerate() {
                bind_pattern(pattern, Term::proj(value.clone(), index), out);
            }
        }
    }
}

fn lower_block(block: &Block) -> Result<exec::Block, LowerError> {
    let mut stmts = Vec::new();
    lower_stmts(&block.stmts, &mut stmts)?;
    let tail = match block.tail.as_deref() {
        None => exec::Tail::Value(unit()),
        Some(tail) => lower_tail(tail, &mut stmts)?,
    };
    Ok(exec::Block { stmts, tail })
}

/// The end of a block: a control transfer, a match whose arms are blocks of
/// the same kind, or a value.
fn lower_tail(tail: &Expr, stmts: &mut Vec<exec::Stmt>) -> Result<exec::Tail, LowerError> {
    Ok(match tail {
        Expr::Break(value) => exec::Tail::Break(anf(value, stmts)?),
        Expr::Continue(next) => exec::Tail::Continue(
            next.iter()
                .map(|expr| anf(expr, stmts))
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
            let scrutinee = anf(&comparison, stmts)?;
            let arms = branches(negated, *then_fact, *else_fact, then_block, else_block)
                .into_iter()
                .map(|(fact, block)| {
                    Ok(Arm {
                        payload: Vec::new(),
                        fact,
                        body: lower_block(block)?,
                    })
                })
                .collect::<Result<_, LowerError>>()?;
            exec::Tail::Match { scrutinee, arms }
        }
        Expr::Match {
            scrutinee, arms, ..
        } if !is_pure(tail) => {
            let scrutinee = anf(scrutinee, stmts)?;
            exec::Tail::Match {
                scrutinee,
                arms: lower_arms(arms)?,
            }
        }
        Expr::Block(block) if !is_pure(tail) => {
            lower_stmts(&block.stmts, stmts)?;
            match block.tail.as_deref() {
                Some(inner) => lower_tail(inner, stmts)?,
                None => exec::Tail::Value(unit()),
            }
        }
        other => exec::Tail::Value(anf(other, stmts)?),
    })
}
