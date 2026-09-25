//! The checking judgments. Each arm of `infer_proof` is one rule of
//! the kernel contract in `atlas.html`.

use super::context::{Context, Mode};
use super::depth::check_depth;
use super::error::KernelError;
use super::eval::{Evaluator, is_plain_data};
use super::int::Integer;
use super::linear::claim_of_linear;
use super::machine::MachineInt;
use super::ops::{Op, Row};
use super::term::{Axiom, ForLoop, HypRef, Prim, Proof, ProofArm, Term, Type, VarId, field_type};

/// The kernel's only comparison of terms: equality up to renaming of bound
/// variables, which the locally nameless representation makes structural, and
/// up to proof irrelevance. Typing guarantees that every proof-typed position
/// inside a term holds a `Term::Proof`, so ignoring those is ignoring exactly
/// the proofs. Nothing is unfolded, evaluated, or normalized.
pub fn same(left: &Term, right: &Term) -> bool {
    let all = |left: &[Term], right: &[Term]| {
        left.len() == right.len() && left.iter().zip(right).all(|(l, r)| same(l, r))
    };
    match (left, right) {
        (
            Term::Buffer {
                op: lo,
                element: le,
                arguments: la,
            },
            Term::Buffer {
                op: ro,
                element: re,
                arguments: ra,
            },
        ) => lo == ro && same_type(le, re) && all(la, ra),
        (Term::Boxed(l), Term::Boxed(r)) => same(l, r),
        (Term::Instance(l, la), Term::Instance(r, ra)) => same(l, r) && all(la, ra),
        (Term::Proof(_), Term::Proof(_)) => true,
        (Term::Free(l), Term::Free(r)) => l == r,
        (Term::Bound(l), Term::Bound(r)) => l == r,
        (Term::Bool(l), Term::Bool(r)) => l == r,
        (Term::U8(l), Term::U8(r)) => l == r,
        // A number has one representation, so this is equality of numbers.
        (Term::Int(l), Term::Int(r)) => l == r,
        (Term::Machine(lt, lv), Term::Machine(rt, rv)) => lt == rt && lv == rv,
        (Term::Prim(lp, la), Term::Prim(rp, ra)) => lp == rp && all(la, ra),
        (Term::Eq(lt, ll, lr), Term::Eq(rt, rl, rr)) => {
            same_type(lt, rt) && same(ll, rl) && same(lr, rr)
        }
        (Term::Implies(lp, lc), Term::Implies(rp, rc)) => same(lp, rp) && same(lc, rc),
        (Term::Forall(lt, lb), Term::Forall(rt, rb)) => same_type(lt, rt) && same(lb, rb),
        (Term::Tuple(lf, lv), Term::Tuple(rf, rv)) => same_types(lf, rf) && all(lv, rv),
        (Term::Struct(li, lv), Term::Struct(ri, rv)) => li == ri && all(lv, rv),
        (Term::Proj(lt, li), Term::Proj(rt, ri)) => li == ri && same(lt, rt),
        (Term::Fn(l), Term::Fn(r)) => l == r,
        (
            Term::Lambda {
                params: lp,
                result: lr,
                body: lb,
            },
            Term::Lambda {
                params: rp,
                result: rr,
                body: rb,
            },
        ) => same_types(lp, rp) && same_type(lr, rr) && same(lb, rb),
        (Term::Call(lc, la), Term::Call(rc, ra)) => same(lc, rc) && all(la, ra),
        (Term::Variant(le, li, lp), Term::Variant(re, ri, rp)) => {
            le == re && li == ri && all(lp, rp)
        }
        (
            Term::Case {
                scrutinee: ls,
                result: lr,
                arms: la,
            },
            Term::Case {
                scrutinee: rs,
                result: rr,
                arms: ra,
            },
        ) => {
            same(ls, rs)
                && same_type(lr, rr)
                && la.len() == ra.len()
                && la
                    .iter()
                    .zip(ra)
                    .all(|(l, r)| l.binders == r.binders && same(&l.body, &r.body))
        }
        (Term::PropApp(li, la), Term::PropApp(ri, ra)) => li == ri && all(la, ra),
        (Term::Exists(lt, lb), Term::Exists(rt, rb)) => same_type(lt, rt) && same(lb, rb),
        // The proof that the bounds are ordered is irrelevant.
        (Term::For(l), Term::For(r)) => {
            same(&l.lo, &r.lo)
                && same(&l.hi, &r.hi)
                && same_types(&l.state, &r.state)
                && same(&l.init, &r.init)
                && same(&l.body, &r.body)
        }
        // Two unreachable values of one type: the proofs are irrelevant.
        (Term::Absurd(_, lt), Term::Absurd(_, rt)) => same_type(lt, rt),
        _ => false,
    }
}

pub fn same_type(left: &Type, right: &Type) -> bool {
    match (left, right) {
        (Type::Boxed(left), Type::Boxed(right)) | (Type::Buffer(left), Type::Buffer(right)) => {
            same_type(left, right)
        }
        (Type::Bool, Type::Bool)
        | (Type::U8, Type::U8)
        | (Type::Int, Type::Int)
        | (Type::Prop, Type::Prop) => true,
        (Type::Machine(l), Type::Machine(r)) => l == r,
        (Type::Instance(l, la), Type::Instance(r, ra)) => {
            same_type(l, r) && la.len() == ra.len() && la.iter().zip(ra).all(|(l, r)| same(l, r))
        }
        (Type::Proof(l), Type::Proof(r)) => same(l, r),
        (Type::Tuple(l), Type::Tuple(r)) => same_types(l, r),
        (Type::Struct(l), Type::Struct(r)) => l == r,
        (Type::Enum(l), Type::Enum(r)) => l == r,
        (Type::Fn(lp, lr), Type::Fn(rp, rr)) => same_types(lp, rp) && same_type(lr, rr),
        _ => false,
    }
}

pub(super) fn same_types(left: &[Type], right: &[Type]) -> bool {
    left.len() == right.len() && left.iter().zip(right).all(|(l, r)| same_type(l, r))
}

/// Checks that a type is well formed in the context.
pub(super) fn type_ok(ctx: &mut Context, ty: &Type) -> Result<(), KernelError> {
    match ty {
        Type::Boxed(element) | Type::Buffer(element) => type_ok(ctx, element),
        Type::Bool | Type::U8 | Type::Int | Type::Prop => Ok(()),
        // `u8` is `Type::U8` and nothing else, so that a type has one form.
        Type::Machine(MachineInt::U8) => Err(KernelError::MachineFormOfU8),
        Type::Machine(_) => Ok(()),
        Type::Proof(prop) => expect_type(ctx, prop, &Type::Prop, Mode::Logical),
        Type::Tuple(fields) => check_telescope(ctx, fields),
        Type::Instance(base, args) => check_family(ctx, base, args),
        Type::Struct(id) => ctx
            .definitions()
            .instance_fields(&Type::Struct(*id))
            .map(|_| ())
            .ok_or(KernelError::UnknownStruct),
        Type::Enum(id) => ctx
            .definitions()
            .instance_variants(&Type::Enum(*id))
            .map(|_| ())
            .ok_or(KernelError::UnknownEnum),
        Type::Fn(params, result) => {
            let mut telescope = params.clone();
            telescope.push((**result).clone());
            check_telescope(ctx, &telescope)
        }
    }
}

fn check_family(ctx: &mut Context, base: &Type, args: &[Term]) -> Result<(), KernelError> {
    let definitions = ctx.definitions();
    let params = definitions
        .family_parameters(base)
        .ok_or_else(|| KernelError::NotAProduct(base.clone()))?;
    if params.is_empty() {
        return Err(KernelError::WrongArity {
            expected: 0,
            found: args.len(),
        });
    }
    check_fields(ctx, params, args, Mode::Logical)
}

fn type_of_instance(
    ctx: &mut Context,
    value: &Term,
    args: &[Term],
    mode: Mode,
) -> Result<Type, KernelError> {
    let base = match value {
        Term::Struct(id, _) => Type::Struct(*id),
        Term::Variant(id, _, _) => Type::Enum(*id),
        _ => return Err(KernelError::NotCaseable(value.clone())),
    };
    check_family(ctx, &base, args)?;
    let ty = Type::Instance(Box::new(base), args.into());
    let definitions = ctx.definitions();
    let (fields, values) = match value {
        Term::Struct(_, values) => (
            definitions
                .instance_fields(&ty)
                .ok_or(KernelError::UnknownStruct)?,
            values,
        ),
        Term::Variant(_, index, values) => {
            let variants = definitions
                .instance_variants(&ty)
                .ok_or(KernelError::UnknownEnum)?;
            (
                variants
                    .get(*index)
                    .cloned()
                    .ok_or(KernelError::NoSuchVariant {
                        index: *index,
                        variants: variants.len(),
                    })?,
                values,
            )
        }
        _ => unreachable!(),
    };
    check_fields(ctx, &fields, values, mode)?;
    Ok(ty)
}

/// Each field type must be well formed given variables for the earlier ones.
pub(super) fn check_telescope(ctx: &mut Context, fields: &[Type]) -> Result<(), KernelError> {
    let scope = ctx.len();
    let mut earlier = Vec::new();
    let mut result = Ok(());
    for index in 0..fields.len() {
        let ty = field_type(fields, index, |j| Term::Free(earlier[j]));
        result = type_ok(ctx, &ty);
        if result.is_err() {
            break;
        }
        earlier.push(ctx.push_bound(ty));
    }
    ctx.truncate(scope);
    result
}

/// Infers the type of a term, rejecting ill-formed terms.
pub(super) fn term_type(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    let found = match term {
        Term::Instance(value, args) => type_of_instance(ctx, value, args, mode),
        Term::Boxed(value) => {
            if mode == Mode::Executable {
                return Err(KernelError::InvalidRecursion(
                    "physical box allocation requires execution IR",
                ));
            }
            let ty = term_type(ctx, value, Mode::Logical)?;
            if matches!(ty, Type::Proof(_)) && !matches!(**value, Term::Proof(_)) {
                return Err(KernelError::ProofExpected((**value).clone()));
            }
            Ok(Type::Boxed(Box::new(ty)))
        }
        Term::Buffer {
            op,
            element,
            arguments,
        } => super::buffer::infer(ctx, *op, element, arguments, mode),
        Term::Free(..) => type_of_free(ctx, term, mode),
        Term::Bound(_) => Err(KernelError::DanglingBound),
        Term::Bool(_) => Ok(Type::Bool),
        Term::U8(_) => Ok(Type::U8),
        Term::Int(_) => ghost_former(mode, &Type::Int).map(|()| Type::Int),
        Term::Machine(..) => type_of_machine(term),
        Term::Prim(..) => type_of_prim(ctx, term, mode),
        Term::Eq(..) => type_of_eq(ctx, term, mode),
        Term::Implies(..) => type_of_implies(ctx, term, mode),
        Term::Forall(..) => type_of_forall(ctx, term, mode),
        Term::Tuple(..) => type_of_tuple(ctx, term, mode),
        Term::Struct(..) => type_of_struct(ctx, term, mode),
        Term::Proj(..) => type_of_proj(ctx, term, mode),
        Term::Proof(..) => type_of_proof(ctx, term, mode),
        Term::Fn(..) => type_of_fn(ctx, term, mode),
        Term::Lambda { .. } => type_of_lambda(ctx, term, mode),
        Term::Call(..) => type_of_call(ctx, term, mode),
        Term::Variant(..) => type_of_variant(ctx, term, mode),
        Term::Case { .. } => type_of_case(ctx, term, mode),
        Term::PropApp(..) => type_of_prop_app(ctx, term, mode),
        Term::Exists(..) => type_of_exists(ctx, term, mode),
        Term::Absurd(..) => type_of_absurd(ctx, term, mode),
        Term::For(..) => type_of_for(ctx, term, mode),
    }?;
    if mode == Mode::Executable && ctx.definitions().is_erased_type(&found) {
        return Err(KernelError::GhostTypeInExecutable(found));
    }
    Ok(found)
}

#[inline(never)]
fn type_of_free(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    let Term::Free(id) = term else {
        unreachable!("dispatched on this variant")
    };
    let (ty, ghost) = ctx.var(*id).ok_or(KernelError::UnknownVariable(*id))?;
    if mode == Mode::Executable && ghost {
        return Err(KernelError::GhostInExecutable(*id));
    }
    Ok(ty.clone())
}

/// A machine integer literal is runtime data in either mode. It is a term
/// only when its value lies in the range of its type, and only at a type
/// other than `u8`, whose literals are `Term::U8`.
#[inline(never)]
fn type_of_lambda(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    if mode == Mode::Executable {
        return Err(KernelError::LogicalFunctionInExecutable);
    }
    let Term::Lambda {
        params,
        result,
        body,
    } = term
    else {
        unreachable!()
    };
    let signature = Type::Fn(params.clone(), Box::new(result.clone()));
    type_ok(ctx, &signature)?;
    let scope = ctx.len();
    let mut values: Vec<Term> = Vec::new();
    let mut fields = params.clone();
    fields.push(result.clone());
    for index in 0..params.len() {
        let ty = field_type(&fields, index, |j| values[j].clone());
        values.push(Term::Free(ctx.push_bound(ty)));
    }
    let expected = field_type(&fields, params.len(), |j| values[j].clone());
    let body = body.instantiate(params.len(), |j| values[j].clone());
    let checked = expect_type(ctx, &body, &expected, Mode::Logical);
    ctx.truncate(scope);
    checked?;
    Ok(signature)
}

#[inline(never)]
fn type_of_machine(term: &Term) -> Result<Type, KernelError> {
    let Term::Machine(ty, value) = term else {
        unreachable!("dispatched on this variant")
    };
    if *ty == MachineInt::U8 {
        return Err(KernelError::MachineFormOfU8);
    }
    if !ty.contains(value) {
        return Err(KernelError::OutOfRange(term.clone()));
    }
    Ok(Type::Machine(*ty))
}

#[inline(never)]
fn type_of_prim(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    let Term::Prim(prim, arguments) = term else {
        unreachable!("dispatched on this variant")
    };
    if let Prim::Op(op, ty) = prim
        && !op.exists_at(*ty)
    {
        return Err(KernelError::NoRow(*op, *ty));
    }
    let (parameters, result) = prim_signature(*prim);
    if arguments.len() != parameters.len() {
        return Err(KernelError::WrongArity {
            expected: parameters.len(),
            found: arguments.len(),
        });
    }
    ghost_former(mode, &result)?;
    for (argument, parameter) in arguments.iter().zip(&parameters) {
        expect_type(ctx, argument, parameter, mode)?;
    }
    Ok(result)
}

#[inline(never)]
fn type_of_eq(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    let Term::Eq(ty, left, right) = term else {
        unreachable!("dispatched on this variant")
    };
    ghost_former(mode, &Type::Prop)?;
    type_ok(ctx, ty)?;
    if matches!(ty, Type::Proof(_)) {
        return Err(KernelError::EqualityAtProofType(ty.clone()));
    }
    expect_type(ctx, left, ty, Mode::Logical)?;
    expect_type(ctx, right, ty, Mode::Logical)?;
    Ok(Type::Prop)
}

#[inline(never)]
fn type_of_implies(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    let Term::Implies(premise, conclusion) = term else {
        unreachable!("dispatched on this variant")
    };
    ghost_former(mode, &Type::Prop)?;
    expect_type(ctx, premise, &Type::Prop, Mode::Logical)?;
    expect_type(ctx, conclusion, &Type::Prop, Mode::Logical)?;
    Ok(Type::Prop)
}

#[inline(never)]
fn type_of_forall(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    let Term::Forall(ty, body) = term else {
        unreachable!("dispatched on this variant")
    };
    ghost_former(mode, &Type::Prop)?;
    type_ok(ctx, ty)?;
    let scope = ctx.len();
    let var = ctx.push_bound(ty.clone());
    let result = expect_type(
        ctx,
        &body.open(&Term::Free(var)),
        &Type::Prop,
        Mode::Logical,
    );
    ctx.truncate(scope);
    result?;
    Ok(Type::Prop)
}

#[inline(never)]
fn type_of_tuple(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    let Term::Tuple(fields, values) = term else {
        unreachable!("dispatched on this variant")
    };
    check_telescope(ctx, fields)?;
    check_fields(ctx, fields, values, mode)?;
    Ok(Type::Tuple(fields.clone()))
}

#[inline(never)]
fn type_of_struct(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    let Term::Struct(id, values) = term else {
        unreachable!("dispatched on this variant")
    };
    let definitions = ctx.definitions();
    let fields = definitions
        .instance_fields(&Type::Struct(*id))
        .ok_or(KernelError::UnknownStruct)?;
    check_fields(ctx, &fields, values, mode)?;
    Ok(Type::Struct(*id))
}

#[inline(never)]
fn type_of_proj(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    let Term::Proj(target, index) = term else {
        unreachable!("dispatched on this variant")
    };
    let target_type = term_type(ctx, target, mode)?;
    let definitions = ctx.definitions();
    let instantiated = definitions.instance_fields(&target_type);
    let fields = match &target_type {
        Type::Boxed(element) => std::slice::from_ref(&**element),
        Type::Tuple(fields) => fields.as_slice(),
        Type::Struct(_) | Type::Instance(..) => {
            instantiated.as_deref().ok_or(KernelError::UnknownStruct)?
        }
        _ => return Err(KernelError::NotAProduct(target_type)),
    };
    if *index >= fields.len() {
        return Err(KernelError::NoSuchField {
            index: *index,
            fields: fields.len(),
        });
    }
    // Earlier fields are named by projecting from the same target,
    // unless the target is literally a product value: then they are
    // its own field values, which is the type the constructor rule
    // checked field `index` against. That keeps the projection axiom
    // well typed for a nested dependent product.
    let ty = match &**target {
        Term::Tuple(_, values) | Term::Struct(_, values) => {
            field_type(fields, *index, |j| values[j].clone())
        }
        _ => field_type(fields, *index, |j| Term::proj((**target).clone(), j)),
    };
    ghost_former(mode, &ty)?;
    Ok(ty)
}

#[inline(never)]
fn type_of_proof(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    let Term::Proof(proof) = term else {
        unreachable!("dispatched on this variant")
    };
    let ty = Type::proof(proof_claim(ctx, proof)?);
    ghost_former(mode, &ty)?;
    Ok(ty)
}

#[inline(never)]
fn type_of_fn(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    let Term::Fn(id) = term else {
        unreachable!("dispatched on this variant")
    };
    let definitions = ctx.definitions();
    let decl = definitions
        .function(*id)
        .ok_or(KernelError::UnknownFunction)?;
    let ty = Type::Fn(decl.params.clone(), Box::new(decl.result.clone()));
    ghost_former(mode, &ty)?;
    // The type alone does not decide this: a function into executable data
    // may still need a ghost value to compute it.
    if mode == Mode::Executable && !decl.executable {
        return Err(KernelError::LogicalFunctionInExecutable);
    }
    Ok(ty)
}

#[inline(never)]
fn type_of_call(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    let Term::Call(callee, arguments) = term else {
        unreachable!("dispatched on this variant")
    };
    let callee_type = term_type(ctx, callee, mode)?;
    let Type::Fn(params, result) = callee_type else {
        return Err(KernelError::NotAFunction(callee_type));
    };
    check_fields(ctx, &params, arguments, mode)?;
    // The result type is the last entry of the parameter telescope.
    let mut telescope = params;
    telescope.push(*result);
    let ty = field_type(&telescope, arguments.len(), |j| arguments[j].clone());
    ghost_former(mode, &ty)?;
    Ok(ty)
}

#[inline(never)]
fn type_of_variant(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    let Term::Variant(id, index, payload) = term else {
        unreachable!("dispatched on this variant")
    };
    let definitions = ctx.definitions();
    let variants = definitions
        .instance_variants(&Type::Enum(*id))
        .ok_or(KernelError::UnknownEnum)?;
    let fields = variants.get(*index).ok_or(KernelError::NoSuchVariant {
        index: *index,
        variants: variants.len(),
    })?;
    check_fields(ctx, fields, payload, mode)?;
    Ok(Type::Enum(*id))
}

#[inline(never)]
fn type_of_case(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    let Term::Case {
        scrutinee,
        result,
        arms,
    } = term
    else {
        unreachable!("dispatched on this variant")
    };
    let scrutinee_type = term_type(ctx, scrutinee, mode)?;
    let variants = data_variants(ctx, &scrutinee_type)
        .ok_or_else(|| KernelError::NotCaseable((**scrutinee).clone()))?;
    type_ok(ctx, result)?;
    if matches!(result, Type::Proof(_)) {
        return Err(KernelError::ProofResult(result.clone()));
    }
    ghost_former(mode, result)?;
    if arms.len() != variants.len() {
        return Err(KernelError::ArmCount {
            expected: variants.len(),
            found: arms.len(),
        });
    }
    for (index, (arm, payload)) in arms.iter().zip(&variants).enumerate() {
        if arm.binders as usize != payload.len() {
            return Err(KernelError::ArmBinders {
                expected: (payload.len(), 0),
                found: (arm.binders as usize, 0),
            });
        }
        let scope = ctx.len();
        let mut vars: Vec<VarId> = Vec::new();
        for index in 0..payload.len() {
            let ty = field_type(payload, index, |j| Term::Free(vars[j]));
            // A payload variable is executable exactly when the case
            // is and its field has a runtime representation.
            let ghost = mode == Mode::Logical || ctx.definitions().is_erased_type(&ty);
            vars.push(ctx.push_local(ty, ghost));
        }
        // The arm knows which variant it has.
        let fact = ctx.push_hyp(Term::eq(
            scrutinee_type.clone(),
            (**scrutinee).clone(),
            constructor_of_vars(&scrutinee_type, index, &vars, payload),
        ));
        let body = arm
            .body
            .instantiate(vars.len(), |j| Term::Free(vars[j]))
            .open_hyps(&[fact]);
        let checked = expect_type(ctx, &body, result, mode);
        ctx.truncate(scope);
        checked?;
    }
    Ok(result.clone())
}

#[inline(never)]
fn type_of_prop_app(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    let Term::PropApp(id, arguments) = term else {
        unreachable!("dispatched on this variant")
    };
    ghost_former(mode, &Type::Prop)?;
    let definitions = ctx.definitions();
    let decl = definitions.prop(*id).ok_or(KernelError::UnknownProp)?;
    if arguments.len() != decl.params.len() {
        return Err(KernelError::FieldCount {
            expected: decl.params.len(),
            found: arguments.len(),
        });
    }
    for (argument, param) in arguments.iter().zip(&decl.params) {
        expect_type(ctx, argument, param, Mode::Logical)?;
    }
    Ok(Type::Prop)
}

#[inline(never)]
fn type_of_exists(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    let Term::Exists(ty, body) = term else {
        unreachable!("dispatched on this variant")
    };
    ghost_former(mode, &Type::Prop)?;
    type_ok(ctx, ty)?;
    let scope = ctx.len();
    let var = ctx.push_bound(ty.clone());
    let result = expect_type(
        ctx,
        &body.open(&Term::Free(var)),
        &Type::Prop,
        Mode::Logical,
    );
    ctx.truncate(scope);
    result?;
    Ok(Type::Prop)
}

#[inline(never)]
fn type_of_absurd(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    let Term::Absurd(proof, ty) = term else {
        unreachable!("dispatched on this variant")
    };
    let prop = proof_claim(ctx, proof)?;
    let empty = match &prop {
        Term::PropApp(id, _) => ctx
            .definitions()
            .prop(*id)
            .is_some_and(|decl| decl.variants.is_empty()),
        _ => false,
    };
    if !empty {
        return Err(KernelError::NotEmpty(prop));
    }
    type_ok(ctx, ty)?;
    ghost_former(mode, ty)?;
    Ok(ty.clone())
}

#[inline(never)]
fn type_of_for(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    let Term::For(looped) = term else {
        unreachable!("dispatched on this variant")
    };
    ctx.definitions().prelude().ok_or(KernelError::NoPrelude)?;
    let (lo, hi) = (&looped.lo, &looped.hi);
    // The bounds have one machine type, read off the lower one.
    let ty = bound_type(ctx, lo, mode)?;
    let index_type = Type::machine(ty);
    expect_type(ctx, hi, &index_type, mode)?;
    let view = |x: &Term| Term::view(ty, x.clone());
    // Ordered bounds make the final index hi, so the result type
    // needs no case distinction.
    proof_of(ctx, &looped.ordered, &Term::int_le(view(lo), view(hi)))?;
    let state_at = |index: &Term| Type::Tuple(looped.state.clone()).open(index);
    expect_type(ctx, &looped.init, &state_at(lo), mode)?;

    let scope = ctx.len();
    let local = mode == Mode::Logical;
    let index = ctx.push_local(index_type, local);
    let i = Term::Free(index);
    let mut checked = type_ok(ctx, &state_at(&i));
    if checked.is_ok() {
        let state = ctx.push_local(state_at(&i), local);
        let lower = ctx.push_hyp(Term::int_le(view(lo), view(&i)));
        let upper = ctx.push_hyp(Term::int_lt(view(&i), view(hi)));
        let body = looped
            .body
            .instantiate(2, |j| Term::Free([index, state][j]))
            .open_hyps(&[lower, upper]);
        // i < hi, so the successor does not wrap.
        let next = Term::successor(ty, i);
        checked = expect_type(ctx, &body, &state_at(&next), mode);
    }
    ctx.truncate(scope);
    checked?;
    Ok(state_at(hi))
}

/// The machine type of a bound of a `for`, which must be a machine integer
/// type; any other type is reported against `u8`, the type the rule was
/// first stated at.
fn bound_type(ctx: &mut Context, bound: &Term, mode: Mode) -> Result<MachineInt, KernelError> {
    let found = term_type(ctx, bound, mode)?;
    found.as_machine().ok_or(KernelError::TypeMismatch {
        expected: Type::U8,
        found,
    })
}

/// The payload telescopes of a data type that supports case analysis:
/// `bool`, with the variants `false` and `true`, or a declared enum.
fn data_variants(ctx: &Context, ty: &Type) -> Option<Vec<Vec<Type>>> {
    match ty {
        Type::Bool => Some(vec![Vec::new(), Vec::new()]),
        Type::Enum(_) | Type::Instance(..) => ctx.definitions().instance_variants(ty),
        _ => None,
    }
}

/// Variant `index` of a case-able data type, applied to a payload.
fn constructor(ty: &Type, index: usize, payload: Vec<Term>) -> Term {
    match ty {
        Type::Instance(base, args) => {
            Term::Instance(Box::new(constructor(base, index, payload)), args.to_vec())
        }
        Type::Enum(id) => Term::Variant(*id, index, payload),
        _ => Term::Bool(index == 1),
    }
}

/// Variant `index` applied to payload variables, as it appears in an arm's
/// fact. A proof field holds `proof(of_term(x))`, not the bare variable: a
/// proof-typed position inside a term always holds a `Term::Proof`, which is
/// what lets comparison ignore proofs without knowing types.
fn constructor_of_vars(ty: &Type, index: usize, vars: &[VarId], payload: &[Type]) -> Term {
    let built = vars
        .iter()
        .zip(payload)
        .map(|(var, field)| match field {
            Type::Proof(_) => Term::proof(Proof::OfTerm(Term::Free(*var))),
            _ => Term::Free(*var),
        })
        .collect();
    constructor(ty, index, built)
}

/// The variant index and payload of a term that is literally a constructor.
fn known_constructor(term: &Term) -> Option<(usize, &[Term])> {
    match term {
        Term::Instance(value, _) => known_constructor(value),
        Term::Bool(value) => Some((usize::from(*value), &[])),
        Term::Variant(_, index, payload) => Some((*index, payload)),
        _ => None,
    }
}

/// Checks one arm of a proof-level case: binds the payload, then the
/// hypotheses the payload gives rise to, and checks the body against the
/// goal. Everything bound is removed again.
fn check_arm(
    ctx: &mut Context,
    arm: &ProofArm,
    payload_len: usize,
    field: impl Fn(usize, &[VarId]) -> Type,
    hypotheses: impl FnMut(&[VarId]) -> Vec<Term>,
    goal: &Term,
) -> Result<(), KernelError> {
    check_arm_with(ctx, arm, payload_len, field, hypotheses, |_| goal.clone())
}

/// As `check_arm`, for a goal that mentions the arm's own variables, which
/// only induction needs.
fn check_arm_with(
    ctx: &mut Context,
    arm: &ProofArm,
    payload_len: usize,
    field: impl Fn(usize, &[VarId]) -> Type,
    mut hypotheses: impl FnMut(&[VarId]) -> Vec<Term>,
    goal: impl FnOnce(&[VarId]) -> Term,
) -> Result<(), KernelError> {
    let scope = ctx.len();
    let mut vars: Vec<VarId> = Vec::new();
    for index in 0..payload_len {
        let ty = field(index, &vars);
        vars.push(ctx.push_bound(ty));
    }
    let hyps: Vec<_> = hypotheses(&vars)
        .into_iter()
        .map(|prop| ctx.push_hyp(prop))
        .collect();
    let goal = goal(&vars);
    let result = if (arm.vars as usize, arm.hyps as usize) == (vars.len(), hyps.len()) {
        proof_of(ctx, &arm.body.open_arm(&vars, &hyps), &goal)
    } else {
        Err(KernelError::ArmBinders {
            expected: (vars.len(), hyps.len()),
            found: (arm.vars as usize, arm.hyps as usize),
        })
    };
    ctx.truncate(scope);
    result
}

fn expect_arm_count(arms: &[ProofArm], variants: usize) -> Result<(), KernelError> {
    if arms.len() == variants {
        Ok(())
    } else {
        Err(KernelError::ArmCount {
            expected: variants,
            found: arms.len(),
        })
    }
}

/// The parameter types and the result type of a primitive.
fn prim_signature(prim: Prim) -> (Vec<Type>, Type) {
    match prim {
        Prim::IntAdd | Prim::IntSub | Prim::IntMul | Prim::IntDiv | Prim::IntRem => {
            (vec![Type::Int, Type::Int], Type::Int)
        }
        Prim::IntNeg => (vec![Type::Int], Type::Int),
        Prim::IntLe => (vec![Type::Int, Type::Int], Type::Prop),
        Prim::IntCmp(_) => (vec![Type::Int, Type::Int], Type::Bool),
        Prim::View(ty) => (vec![Type::machine(ty)], Type::Int),
        Prim::Wrap(ty) => (vec![Type::Int], Type::machine(ty)),
        Prim::Cast(from, to) => (vec![Type::machine(from)], Type::machine(to)),
        // A row that does not exist is rejected before this is asked.
        Prim::Op(op, ty) => (vec![Type::machine(ty); op.arity()], Type::machine(ty)),
        Prim::Cmp(_, ty) => (vec![Type::machine(ty); 2], Type::Bool),
    }
}

/// Native evaluation of a primitive applied to literals. This is the
/// implementation that must agree with the integers as a model of the `Int`
/// axioms, with the machine integers as a model of the axioms about `view`,
/// `wrap`, and `cast`, with the table of primitive operations, and with
/// Rust's comparisons. `int_le` is a proposition and has no value;
/// `evaluate` decides it.
pub fn evaluate_primitive(prim: Prim, arguments: &[Term]) -> Option<Term> {
    // A machine literal of the type the primitive expects, as a number.
    let machine = |ty: MachineInt, term: &Term| -> Option<Integer> {
        term.machine_value()
            .and_then(|(found, value)| (found == ty).then_some(value))
    };
    Some(match (prim, arguments) {
        (Prim::View(ty), [x]) => Term::Int(machine(ty, x)?),
        (Prim::Wrap(ty), [Term::Int(n)]) => Term::machine(ty, ty.wrap(n)),
        // `x as T` is `wrap(T)` of the value of `x`, by `cast_def`.
        (Prim::Cast(from, to), [x]) => Term::machine(to, to.wrap(&machine(from, x)?)),
        // A row of the table: the meaning that holds in every build, which
        // is total, so nothing here panics.
        (Prim::Op(op, ty), operands) => {
            let row = op.row(ty)?;
            if operands.len() != row.arity() {
                return None;
            }
            let values: Vec<Integer> = operands
                .iter()
                .map(|operand| machine(ty, operand))
                .collect::<Option<_>>()?;
            Term::machine(ty, row.compute(&values))
        }
        // A comparison of two values of a type is the comparison of their
        // numbers, which is what `cmp_reflect` states of the views.
        (Prim::Cmp(op, ty), [a, b]) => Term::Bool(op.holds(&machine(ty, a)?, &machine(ty, b)?)),
        (Prim::IntCmp(op), [Term::Int(a), Term::Int(b)]) => Term::Bool(op.holds(a, b)),
        (Prim::IntAdd, [Term::Int(a), Term::Int(b)]) => Term::Int(a.add(b)),
        (Prim::IntSub, [Term::Int(a), Term::Int(b)]) => Term::Int(a.sub(b)),
        (Prim::IntMul, [Term::Int(a), Term::Int(b)]) => Term::Int(a.mul(b)),
        // Truncated toward zero and total: a / 0 is 0 and a % 0 is a.
        (Prim::IntDiv, [Term::Int(a), Term::Int(b)]) => Term::Int(a.div(b)),
        (Prim::IntRem, [Term::Int(a), Term::Int(b)]) => Term::Int(a.rem(b)),
        (Prim::IntNeg, [Term::Int(a)]) => Term::Int(a.neg()),
        _ => return None,
    })
}

/// The proposition an axiom states, after typing its arguments.
fn axiom_statement(ctx: &mut Context, axiom: &Axiom) -> Result<Term, KernelError> {
    let prelude = ctx.definitions().prelude().ok_or(KernelError::NoPrelude)?;
    let expected = match axiom {
        Axiom::IntAddAssoc(..)
        | Axiom::IntAddComm(..)
        | Axiom::IntAddZero(_)
        | Axiom::IntAddNeg(_)
        | Axiom::IntSubDef(..)
        | Axiom::IntMulAssoc(..)
        | Axiom::IntMulComm(..)
        | Axiom::IntMulOne(_)
        | Axiom::IntMulAdd(..)
        | Axiom::IntLeRefl(_)
        | Axiom::IntLeTrans(..)
        | Axiom::IntLeAntisymm(..)
        | Axiom::IntLeAdd(..)
        | Axiom::IntLeMul(..)
        | Axiom::IntLeTotal(..)
        | Axiom::IntLtIrrefl(_)
        | Axiom::IntDivRem(..)
        | Axiom::IntDivZero(_)
        | Axiom::IntRemLowerPos(..)
        | Axiom::IntRemUpperPos(..)
        | Axiom::IntRemLowerNeg(..)
        | Axiom::IntRemUpperNeg(..)
        | Axiom::IntRemNonneg(..)
        | Axiom::IntRemNonpos(..)
        | Axiom::ViewWrap(..)
        | Axiom::WrapPeriod(..) => Some(Type::Int),
        Axiom::ViewLower(ty, _)
        | Axiom::ViewUpper(ty, _)
        | Axiom::WrapView(ty, _)
        | Axiom::CastDef(ty, _, _)
        | Axiom::OpModel(_, ty, _)
        | Axiom::OpExact(_, ty, _) => Some(Type::machine(*ty)),
        Axiom::CmpReflect(..) | Axiom::CmpReify(..) => None,
    };
    if let Some(expected) = &expected {
        for term in axiom.terms() {
            expect_type(ctx, term, expected, Mode::Logical)?;
        }
    }
    let int_eq = |left: Term, right: Term| Term::eq(Type::Int, left, right);
    let (add, mul, le) = (Term::int_add, Term::int_mul, Term::int_le);
    let (div, rem, lt) = (Term::int_div, Term::int_rem, Term::int_lt);
    Ok(match axiom.clone() {
        Axiom::IntAddAssoc(a, b, c) => {
            int_eq(add(add(a.clone(), b.clone()), c.clone()), add(a, add(b, c)))
        }
        Axiom::IntAddComm(a, b) => int_eq(add(a.clone(), b.clone()), add(b, a)),
        Axiom::IntAddZero(a) => int_eq(add(a.clone(), Term::int(0)), a),
        Axiom::IntAddNeg(a) => int_eq(add(a.clone(), Term::int_neg(a)), Term::int(0)),
        Axiom::IntSubDef(a, b) => int_eq(
            Term::int_sub(a.clone(), b.clone()),
            add(a, Term::int_neg(b)),
        ),
        Axiom::IntMulAssoc(a, b, c) => {
            int_eq(mul(mul(a.clone(), b.clone()), c.clone()), mul(a, mul(b, c)))
        }
        Axiom::IntMulComm(a, b) => int_eq(mul(a.clone(), b.clone()), mul(b, a)),
        Axiom::IntMulOne(a) => int_eq(mul(a.clone(), Term::int(1)), a),
        Axiom::IntMulAdd(a, b, c) => int_eq(
            mul(a.clone(), add(b.clone(), c.clone())),
            add(mul(a.clone(), b), mul(a, c)),
        ),
        Axiom::IntLeRefl(a) => le(a.clone(), a),
        Axiom::IntLeTrans(a, b, c) => Term::implies(
            le(a.clone(), b.clone()),
            Term::implies(le(b, c.clone()), le(a, c)),
        ),
        Axiom::IntLeAntisymm(a, b) => Term::implies(
            le(a.clone(), b.clone()),
            Term::implies(le(b.clone(), a.clone()), int_eq(a, b)),
        ),
        Axiom::IntLeAdd(a, b, c) => {
            Term::implies(le(a.clone(), b.clone()), le(add(a, c.clone()), add(b, c)))
        }
        Axiom::IntLeMul(a, b) => Term::implies(
            le(Term::int(0), a.clone()),
            Term::implies(le(Term::int(0), b.clone()), le(Term::int(0), mul(a, b))),
        ),
        // The second case is b < a written out, so this one axiom says the
        // order is total and that nothing lies between b and b + 1.
        Axiom::IntLeTotal(a, b) => prelude.or_prop(le(a.clone(), b.clone()), Term::int_lt(b, a)),
        Axiom::IntLtIrrefl(a) => prelude.not_prop(Term::int_lt(a.clone(), a)),
        // Quotient and remainder, truncated toward zero. The decomposition
        // has no condition: at b == 0 it reads a == 0 * 0 + a.
        Axiom::IntDivRem(a, b) => int_eq(
            a.clone(),
            add(mul(div(a.clone(), b.clone()), b.clone()), rem(a, b)),
        ),
        Axiom::IntDivZero(a) => int_eq(div(a, Term::int(0)), Term::int(0)),
        // The remainder is smaller in magnitude than the divisor. Each bound
        // needs its condition: at b == 0 the remainder is a, and no bound
        // holds of every a.
        Axiom::IntRemLowerPos(a, b) => Term::implies(
            lt(Term::int(0), b.clone()),
            lt(Term::int_neg(b.clone()), rem(a, b)),
        ),
        Axiom::IntRemUpperPos(a, b) => {
            Term::implies(lt(Term::int(0), b.clone()), lt(rem(a, b.clone()), b))
        }
        Axiom::IntRemLowerNeg(a, b) => {
            Term::implies(lt(b.clone(), Term::int(0)), lt(b.clone(), rem(a, b)))
        }
        Axiom::IntRemUpperNeg(a, b) => Term::implies(
            lt(b.clone(), Term::int(0)),
            lt(rem(a, b.clone()), Term::int_neg(b)),
        ),
        // The remainder has the sign of the dividend, at every divisor.
        Axiom::IntRemNonneg(a, b) => {
            Term::implies(le(Term::int(0), a.clone()), le(Term::int(0), rem(a, b)))
        }
        Axiom::IntRemNonpos(a, b) => {
            Term::implies(le(a.clone(), Term::int(0)), le(rem(a, b), Term::int(0)))
        }
        // The model of a machine type T over Int. The two round trips and
        // the period say wrap is reduction modulo 2^bits into [min, max].
        Axiom::ViewLower(ty, x) => le(Term::Int(ty.min()), Term::view(ty, x)),
        Axiom::ViewUpper(ty, x) => le(Term::view(ty, x), Term::Int(ty.max())),
        Axiom::WrapView(ty, x) => Term::eq(
            Type::machine(ty),
            Term::wrap(ty, Term::view(ty, x.clone())),
            x,
        ),
        Axiom::ViewWrap(ty, n) => Term::implies(
            le(Term::Int(ty.min()), n.clone()),
            Term::implies(
                le(n.clone(), Term::Int(ty.max())),
                int_eq(Term::view(ty, Term::wrap(ty, n.clone())), n),
            ),
        ),
        Axiom::WrapPeriod(ty, n) => Term::eq(
            Type::machine(ty),
            Term::wrap(ty, add(n.clone(), Term::Int(ty.modulus()))),
            Term::wrap(ty, n),
        ),
        Axiom::CastDef(from, to, x) => Term::eq(
            Type::machine(to),
            Term::cast(from, to, x.clone()),
            Term::wrap(to, Term::view(from, x)),
        ),
        // The table of primitive operations: each schema is stated by the
        // row it names, in `src/kernel/ops.rs`.
        Axiom::OpModel(op, ty, operands) => {
            table_row(op, ty, &operands)?.model_statement(&operands)
        }
        Axiom::OpExact(op, ty, operands) => table_row(op, ty, &operands)?
            .exact_statement(&operands)
            .ok_or(KernelError::NoOverflow(op, ty))?,
        // Reflection at every machine type: the comparison decides the
        // proposition of the same name about the views. Typing the
        // comparison types its operands at the type it carries.
        Axiom::CmpReflect(comparison, flag) | Axiom::CmpReify(comparison, flag) => {
            expect_type(ctx, &comparison, &Type::Bool, Mode::Logical)?;
            let claim = comparison_claim(&comparison)
                .ok_or_else(|| KernelError::NoComputationStep(comparison.clone()))?;
            let observed = Term::eq(Type::Bool, comparison, Term::Bool(flag));
            let claim = if flag { claim } else { prelude.not_prop(claim) };
            if matches!(axiom, Axiom::CmpReify(..)) {
                Term::implies(claim, observed)
            } else {
                Term::implies(observed, claim)
            }
        }
    })
}

/// The proposition a runtime comparison at a machine type decides, over
/// the views of its operands.
fn comparison_claim(comparison: &Term) -> Option<Term> {
    let Term::Prim(prim, arguments) = comparison else {
        return None;
    };
    let [left, right] = arguments.as_slice() else {
        return None;
    };
    match prim {
        Prim::Cmp(op, ty) => Some(op.claim(
            Term::view(*ty, left.clone()),
            Term::view(*ty, right.clone()),
        )),
        Prim::IntCmp(op) => Some(op.claim(left.clone(), right.clone())),
        _ => None,
    }
}

/// The row an axiom about the table names, when it exists and the axiom
/// has as many operands as the row.
fn table_row(op: Op, ty: MachineInt, operands: &[Term]) -> Result<Row, KernelError> {
    let row = op.row(ty).ok_or(KernelError::NoRow(op, ty))?;
    if operands.len() != row.arity() {
        return Err(KernelError::WrongArity {
            expected: row.arity(),
            found: operands.len(),
        });
    }
    Ok(row)
}

/// A term of a ghost type has no runtime value, so it is never executable.
fn ghost_former(mode: Mode, ty: &Type) -> Result<(), KernelError> {
    if mode == Mode::Executable && ty.is_ghost() {
        Err(KernelError::GhostTypeInExecutable(ty.clone()))
    } else {
        Ok(())
    }
}

/// Checks field values in order. Each value is checked against its field's
/// type with the earlier values substituted in. A ghost field is a logical
/// position even inside an executable product.
fn check_fields(
    ctx: &mut Context,
    fields: &[Type],
    values: &[Term],
    mode: Mode,
) -> Result<(), KernelError> {
    if fields.len() != values.len() {
        return Err(KernelError::FieldCount {
            expected: fields.len(),
            found: values.len(),
        });
    }
    for (index, value) in values.iter().enumerate() {
        let expected = field_type(fields, index, |j| values[j].clone());
        match &expected {
            Type::Proof(prop) => {
                let Term::Proof(proof) = value else {
                    return Err(KernelError::ProofExpected(value.clone()));
                };
                proof_of(ctx, proof, prop)?;
            }
            _ if ctx.definitions().is_erased_type(&expected) => {
                expect_type(ctx, value, &expected, Mode::Logical)?
            }
            _ => expect_type(ctx, value, &expected, mode)?,
        }
    }
    Ok(())
}

pub(super) fn expect_type(
    ctx: &mut Context,
    term: &Term,
    expected: &Type,
    mode: Mode,
) -> Result<(), KernelError> {
    let found = term_type(ctx, term, mode)?;
    if same_type(&found, expected) {
        Ok(())
    } else {
        Err(KernelError::TypeMismatch {
            expected: expected.clone(),
            found,
        })
    }
}

/// Reads off the proposition a proof proves, checking every step.
/// The result is well formed in `ctx`.
pub(super) fn proof_claim(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    match proof {
        Proof::BufferStep(term) => super::buffer::proof_step(ctx, term),
        Proof::BufferBound { value, upper } => super::buffer::proof_bound(ctx, value, *upper),
        Proof::Hyp(HypRef::Free(id)) => ctx
            .hyp(*id)
            .cloned()
            .ok_or(KernelError::UnknownHypothesis(*id)),
        Proof::Hyp(HypRef::Bound(_)) => Err(KernelError::DanglingBound),
        Proof::OfTerm(term) => match term_type(ctx, term, Mode::Logical)? {
            Type::Proof(prop) => Ok(*prop),
            other => Err(KernelError::NotAProofType(other)),
        },
        Proof::Refl(..) => claim_of_refl(ctx, proof),
        Proof::Transport { .. } => claim_of_transport(ctx, proof),
        Proof::ImpliesIntro { .. } => claim_of_implies_intro(ctx, proof),
        Proof::ImpliesElim(..) => claim_of_implies_elim(ctx, proof),
        Proof::ForallIntro { .. } => claim_of_forall_intro(ctx, proof),
        Proof::ForallElim(..) => claim_of_forall_elim(ctx, proof),
        Proof::Projection(..) => claim_of_projection(ctx, proof),
        Proof::Literal(..) => claim_of_literal(ctx, proof),
        Proof::Definition(..) => claim_of_definition(ctx, proof),
        Proof::CaseStep(..) => claim_of_case_step(ctx, proof),
        Proof::CaseKnown { term, equation } => claim_of_case_known(ctx, term, equation),
        Proof::Construct { .. } => claim_of_construct(ctx, proof),
        Proof::CaseProof { .. } => claim_of_case_proof(ctx, proof),
        Proof::CaseData { .. } => claim_of_case_data(ctx, proof),
        Proof::ExistsIntro { .. } => claim_of_exists_intro(ctx, proof),
        Proof::ExistsElim { .. } => claim_of_exists_elim(ctx, proof),
        Proof::ExcludedMiddle(..) => claim_of_excluded_middle(ctx, proof),
        Proof::ForEmpty(..) => claim_of_for_empty(ctx, proof),
        Proof::ForStep { .. } => claim_of_for_step(ctx, proof),
        Proof::Omitted => Err(KernelError::OmittedProof),
        Proof::Evaluate(..) => claim_of_evaluate(ctx, proof),
        Proof::Axiom(axiom) => axiom_statement(ctx, axiom),
        Proof::IntInduction { .. } => claim_of_int_induction(ctx, proof),
        Proof::DataInduction { .. } => claim_of_data_induction(ctx, proof),
        Proof::PropInduction { .. } => claim_of_prop_induction(ctx, proof),
        Proof::Linear { .. } => claim_of_linear(ctx, proof),
    }
}

#[inline(never)]
fn claim_of_refl(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::Refl(term) = proof else {
        unreachable!("dispatched on this variant")
    };
    let ty = term_type(ctx, term, Mode::Logical)?;
    if matches!(ty, Type::Proof(_)) {
        return Err(KernelError::EqualityAtProofType(ty));
    }
    Ok(Term::eq(ty, term.clone(), term.clone()))
}

#[inline(never)]
fn claim_of_transport(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::Transport {
        eq,
        template,
        proof,
    } = proof
    else {
        unreachable!("dispatched on this variant")
    };
    let equality = proof_claim(ctx, eq)?;
    let Term::Eq(ty, left, right) = equality else {
        return Err(KernelError::NotAnEquality(equality));
    };
    let scope = ctx.len();
    let hole = ctx.push_bound(ty);
    let well_formed = expect_type(
        ctx,
        &template.open(&Term::Free(hole)),
        &Type::Prop,
        Mode::Logical,
    );
    ctx.truncate(scope);
    well_formed?;
    proof_of(ctx, proof, &template.open(&left))?;
    Ok(template.open(&right))
}

#[inline(never)]
fn claim_of_implies_intro(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::ImpliesIntro { hyp, body } = proof else {
        unreachable!("dispatched on this variant")
    };
    expect_type(ctx, hyp, &Type::Prop, Mode::Logical)?;
    let scope = ctx.len();
    let id = ctx.push_hyp(hyp.clone());
    let conclusion = proof_claim(ctx, &body.open_hyp(id));
    ctx.truncate(scope);
    Ok(Term::implies(hyp.clone(), conclusion?))
}

#[inline(never)]
fn claim_of_implies_elim(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::ImpliesElim(implication, premise) = proof else {
        unreachable!("dispatched on this variant")
    };
    let prop = proof_claim(ctx, implication)?;
    let Term::Implies(expected, conclusion) = prop else {
        return Err(KernelError::NotAnImplication(prop));
    };
    proof_of(ctx, premise, &expected)?;
    Ok(*conclusion)
}

#[inline(never)]
fn claim_of_forall_intro(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::ForallIntro { ty, body } = proof else {
        unreachable!("dispatched on this variant")
    };
    type_ok(ctx, ty)?;
    let scope = ctx.len();
    let var = ctx.push_bound(ty.clone());
    let instance = proof_claim(ctx, &body.open_var(&Term::Free(var)));
    ctx.truncate(scope);
    Ok(Term::Forall(ty.clone(), Box::new(instance?.close(var))))
}

#[inline(never)]
fn claim_of_forall_elim(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::ForallElim(universal, argument) = proof else {
        unreachable!("dispatched on this variant")
    };
    let prop = proof_claim(ctx, universal)?;
    let Term::Forall(ty, body) = prop else {
        return Err(KernelError::NotUniversal(prop));
    };
    expect_type(ctx, argument, &ty, Mode::Logical)?;
    Ok(body.open(argument))
}

#[inline(never)]
fn claim_of_projection(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::Projection(term) = proof else {
        unreachable!("dispatched on this variant")
    };
    let Term::Proj(target, index) = term else {
        return Err(KernelError::NoComputationStep(term.clone()));
    };
    let raw = match &**target {
        Term::Instance(value, _) => &**value,
        other => other,
    };
    let values: &[Term] = match raw {
        Term::Tuple(_, values) | Term::Struct(_, values) => values,
        Term::Boxed(value) => std::slice::from_ref(&**value),
        _ => return Err(KernelError::NoComputationStep(term.clone())),
    };
    let ty = term_type(ctx, term, Mode::Logical)?;
    if matches!(ty, Type::Proof(_)) {
        return Err(KernelError::EqualityAtProofType(ty));
    }
    // Projection from a literal product is typed by the product's
    // own field values, so the value has exactly this type.
    let value = &values[*index];
    if !same_type(&term_type(ctx, value, Mode::Logical)?, &ty) {
        return Err(KernelError::NoComputationStep(term.clone()));
    }
    Ok(Term::eq(ty, term.clone(), value.clone()))
}

#[inline(never)]
fn claim_of_literal(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::Literal(term) = proof else {
        unreachable!("dispatched on this variant")
    };
    let Term::Prim(prim, arguments) = term else {
        return Err(KernelError::NoComputationStep(term.clone()));
    };
    let value = evaluate_primitive(*prim, arguments)
        .ok_or_else(|| KernelError::NoComputationStep(term.clone()))?;
    let ty = term_type(ctx, term, Mode::Logical)?;
    Ok(Term::eq(ty, term.clone(), value))
}

#[inline(never)]
fn claim_of_definition(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::Definition(term) = proof else {
        unreachable!("dispatched on this variant")
    };
    let Term::Call(callee, arguments) = term else {
        return Err(KernelError::NoComputationStep(term.clone()));
    };
    let ty = term_type(ctx, term, Mode::Logical)?;
    if matches!(ty, Type::Proof(_)) {
        return Err(KernelError::EqualityAtProofType(ty));
    }
    let definitions = ctx.definitions();
    let body = match &**callee {
        Term::Fn(id) => {
            &definitions
                .function(*id)
                .ok_or(KernelError::UnknownFunction)?
                .body
        }
        Term::Lambda { body, .. } => body,
        _ => return Err(KernelError::NoComputationStep(term.clone())),
    };
    let unfolded = body.instantiate(arguments.len(), |j| arguments[j].clone());
    Ok(Term::eq(ty, term.clone(), unfolded))
}

#[inline(never)]
fn claim_of_case_step(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::CaseStep(term) = proof else {
        unreachable!("dispatched on this variant")
    };
    let Term::Case {
        scrutinee, arms, ..
    } = term
    else {
        return Err(KernelError::NoComputationStep(term.clone()));
    };
    let Some((index, payload)) = known_constructor(scrutinee) else {
        return Err(KernelError::NoComputationStep(term.clone()));
    };
    let ty = term_type(ctx, term, Mode::Logical)?;
    // The arm's fact, scrutinee == variant(payload), holds by reflexivity.
    let fact = Proof::Refl((**scrutinee).clone());
    let chosen = arms[index]
        .body
        .instantiate(payload.len(), |j| payload[j].clone())
        .subst_hyps(&[&fact]);
    Ok(Term::eq(ty, term.clone(), chosen))
}

#[inline(never)]
fn claim_of_case_known(
    ctx: &mut Context,
    term: &Term,
    equation: &Proof,
) -> Result<Term, KernelError> {
    let Term::Case {
        scrutinee, arms, ..
    } = term
    else {
        return Err(KernelError::NoComputationStep(term.clone()));
    };
    let ty = term_type(ctx, term, Mode::Logical)?;
    let scrutinee_type = term_type(ctx, scrutinee, Mode::Logical)?;
    let claim = proof_claim(ctx, equation)?;
    let Term::Eq(eq_type, actual, constructor) = &claim else {
        return Err(KernelError::NotAnEquality(claim));
    };
    if !same_type(eq_type, &scrutinee_type) || !same(actual, scrutinee) {
        return Err(KernelError::NoComputationStep(term.clone()));
    }
    let Some((index, payload)) = known_constructor(constructor) else {
        return Err(KernelError::NoComputationStep(term.clone()));
    };
    let arm = arms
        .get(index)
        .ok_or(KernelError::NoComputationStep(term.clone()))?;
    let chosen = arm
        .body
        .instantiate(payload.len(), |j| payload[j].clone())
        .subst_hyps(&[equation]);
    expect_type(ctx, &chosen, &ty, Mode::Logical)?;
    Ok(Term::eq(ty, term.clone(), chosen))
}

#[inline(never)]
fn claim_of_construct(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::Construct {
        prop,
        variant,
        params,
        payload,
    } = proof
    else {
        unreachable!("dispatched on this variant")
    };
    let definitions = ctx.definitions();
    let decl = definitions.prop(*prop).ok_or(KernelError::UnknownProp)?;
    let chosen = decl
        .variants
        .get(*variant)
        .ok_or(KernelError::NoSuchVariant {
            index: *variant,
            variants: decl.variants.len(),
        })?;
    let expected_params = if chosen.with_params {
        decl.params.len()
    } else {
        0
    };
    if params.len() != expected_params {
        return Err(KernelError::FieldCount {
            expected: expected_params,
            found: params.len(),
        });
    }
    let values: Vec<Term> = params.iter().chain(payload).cloned().collect();
    check_fields(ctx, &chosen.telescope, &values, Mode::Logical)?;
    let arguments = if chosen.with_params {
        params.clone()
    } else {
        chosen
            .conclusion
            .iter()
            .map(|argument| argument.instantiate(payload.len(), |j| payload[j].clone()))
            .collect()
    };
    Ok(Term::PropApp(*prop, arguments))
}

#[inline(never)]
fn claim_of_case_proof(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::CaseProof {
        scrutinee,
        goal,
        arms,
    } = proof
    else {
        unreachable!("dispatched on this variant")
    };
    let proved = proof_claim(ctx, scrutinee)?;
    let Term::PropApp(id, arguments) = &proved else {
        return Err(KernelError::NotCaseable(proved));
    };
    expect_type(ctx, goal, &Type::Prop, Mode::Logical)?;
    let definitions = ctx.definitions();
    let decl = definitions.prop(*id).ok_or(KernelError::UnknownProp)?;
    expect_arm_count(arms, decl.variants.len())?;
    for (arm, variant) in arms.iter().zip(&decl.variants) {
        if variant.with_params {
            // The parameters are the scrutinee's own arguments, so
            // there is nothing to equate.
            let params = decl.params.len();
            check_arm(
                ctx,
                arm,
                variant.telescope.len() - params,
                |index, vars| {
                    field_type(&variant.telescope, params + index, |j| {
                        if j < params {
                            arguments[j].clone()
                        } else {
                            Term::Free(vars[j - params])
                        }
                    })
                },
                |_| Vec::new(),
                goal,
            )?;
        } else {
            // One index equation per parameter: the scrutinee's
            // argument equals the variant's stated one.
            check_arm(
                ctx,
                arm,
                variant.telescope.len(),
                |index, vars| field_type(&variant.telescope, index, |j| Term::Free(vars[j])),
                |vars| {
                    let stated = variant
                        .conclusion
                        .iter()
                        .map(|term| term.instantiate(vars.len(), |j| Term::Free(vars[j])));
                    decl.params
                        .iter()
                        .zip(arguments)
                        .zip(stated)
                        .map(|((ty, actual), stated)| Term::eq(ty.clone(), actual.clone(), stated))
                        .collect()
                },
                goal,
            )?;
        }
    }
    Ok(goal.clone())
}

#[inline(never)]
fn claim_of_case_data(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::CaseData {
        scrutinee,
        goal,
        arms,
    } = proof
    else {
        unreachable!("dispatched on this variant")
    };
    let ty = term_type(ctx, scrutinee, Mode::Logical)?;
    let variants =
        data_variants(ctx, &ty).ok_or_else(|| KernelError::NotCaseable(scrutinee.clone()))?;
    expect_type(ctx, goal, &Type::Prop, Mode::Logical)?;
    expect_arm_count(arms, variants.len())?;
    for (index, (arm, payload)) in arms.iter().zip(&variants).enumerate() {
        check_arm(
            ctx,
            arm,
            payload.len(),
            |field, vars| field_type(payload, field, |j| Term::Free(vars[j])),
            |vars| {
                vec![Term::eq(
                    ty.clone(),
                    scrutinee.clone(),
                    constructor_of_vars(&ty, index, vars, payload),
                )]
            },
            goal,
        )?;
    }
    Ok(goal.clone())
}

#[inline(never)]
fn claim_of_exists_intro(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::ExistsIntro {
        prop,
        witness,
        proof,
    } = proof
    else {
        unreachable!("dispatched on this variant")
    };
    expect_type(ctx, prop, &Type::Prop, Mode::Logical)?;
    let Term::Exists(ty, body) = prop else {
        return Err(KernelError::NotExistential(prop.clone()));
    };
    expect_type(ctx, witness, ty, Mode::Logical)?;
    proof_of(ctx, proof, &body.open(witness))?;
    Ok(prop.clone())
}

#[inline(never)]
fn claim_of_exists_elim(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::ExistsElim { exists, goal, arm } = proof else {
        unreachable!("dispatched on this variant")
    };
    let proved = proof_claim(ctx, exists)?;
    let Term::Exists(ty, body) = &proved else {
        return Err(KernelError::NotExistential(proved));
    };
    // The goal is checked before the witness exists, so it cannot
    // mention the witness.
    expect_type(ctx, goal, &Type::Prop, Mode::Logical)?;
    check_arm(
        ctx,
        arm,
        1,
        |_, _| ty.clone(),
        |vars| vec![body.open(&Term::Free(vars[0]))],
        goal,
    )?;
    Ok(goal.clone())
}

#[inline(never)]
fn claim_of_excluded_middle(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::ExcludedMiddle(prop) = proof else {
        unreachable!("dispatched on this variant")
    };
    let prelude = ctx.definitions().prelude().ok_or(KernelError::NoPrelude)?;
    expect_type(ctx, prop, &Type::Prop, Mode::Logical)?;
    Ok(prelude.or_prop(prop.clone(), prelude.not_prop(prop.clone())))
}

#[inline(never)]
fn claim_of_for_empty(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::ForEmpty(term) = proof else {
        unreachable!("dispatched on this variant")
    };
    let Term::For(looped) = term else {
        return Err(KernelError::NoComputationStep(term.clone()));
    };
    if !same(&looped.lo, &looped.hi) {
        return Err(KernelError::NoComputationStep(term.clone()));
    }
    let ty = term_type(ctx, term, Mode::Logical)?;
    Ok(Term::eq(ty, term.clone(), looped.init.clone()))
}

#[inline(never)]
fn claim_of_for_step(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::ForStep {
        looped,
        lower,
        upper,
    } = proof
    else {
        unreachable!("dispatched on this variant")
    };
    let no_step = || KernelError::NoComputationStep(looped.clone());
    let Term::For(this) = looped else {
        return Err(no_step());
    };
    // The upper bound is written as the successor of h at the index type.
    let Term::Prim(Prim::Op(Op::WrappingAdd, index_type), bound) = &this.hi else {
        return Err(no_step());
    };
    let [h, one] = bound.as_slice() else {
        return Err(no_step());
    };
    if *one != Term::machine_int(*index_type, 1) {
        return Err(no_step());
    }
    ctx.definitions().prelude().ok_or(KernelError::NoPrelude)?;
    let ty = term_type(ctx, looped, Mode::Logical)?;
    let view = |x: &Term| Term::view(*index_type, x.clone());
    proof_of(ctx, lower, &Term::int_le(view(&this.lo), view(h)))?;
    proof_of(ctx, upper, &Term::int_lt(view(h), view(&this.hi)))?;
    // The loop up to h. Its body is the same term, now read under the
    // hypothesis i < h; a body whose proofs rely on the old upper
    // bound does not type-check here, and then there is no step.
    let previous = Term::For(Box::new(ForLoop {
        hi: h.clone(),
        ordered: (**lower).clone(),
        ..(**this).clone()
    }));
    let arguments = [h.clone(), previous];
    let unrolled = this
        .body
        .instantiate(2, |j| arguments[j].clone())
        .subst_hyps(&[&**lower, &**upper]);
    match term_type(ctx, &unrolled, Mode::Logical) {
        Ok(found) if same_type(&found, &ty) => Ok(Term::eq(ty, looped.clone(), unrolled)),
        _ => Err(no_step()),
    }
}

#[inline(never)]
fn claim_of_evaluate(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::Evaluate(term) = proof else {
        unreachable!("dispatched on this variant")
    };
    let ty = term_type(ctx, term, Mode::Logical)?;
    let definitions = ctx.definitions();
    // A comparison of integers is a proposition, so it has no value to be
    // equal to. It is decided instead: evaluation proves the comparison
    // when it holds of the two values, and its negation when it does not.
    let sides = match term {
        Term::Prim(Prim::IntLe, sides) => match sides.as_slice() {
            [left, right] => Some((left, right)),
            _ => None,
        },
        Term::Eq(Type::Int, left, right) => Some((&**left, &**right)),
        _ => None,
    };
    if let Some((left, right)) = sides {
        // One evaluator: the budget covers both sides.
        let mut evaluator = Evaluator::new(&definitions);
        let (Term::Int(l), Term::Int(r)) = (evaluator.eval(left)?, evaluator.eval(right)?) else {
            return Err(KernelError::NoComputationStep(term.clone()));
        };
        let holds = if matches!(term, Term::Eq(..)) {
            l == r
        } else {
            l <= r
        };
        return if holds {
            Ok(term.clone())
        } else {
            let prelude = definitions.prelude().ok_or(KernelError::NoPrelude)?;
            Ok(prelude.not_prop(term.clone()))
        };
    }
    if !is_plain_data(&definitions, &ty) {
        return Err(KernelError::NotPlainData(ty));
    }
    let value = Evaluator::new(&definitions).eval(term)?;
    Ok(Term::eq(ty, term.clone(), value))
}

#[inline(never)]
fn claim_of_int_induction(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::IntInduction {
        motive,
        base,
        step,
        target,
    } = proof
    else {
        unreachable!("dispatched on this variant")
    };
    let scope = ctx.len();
    let hole = ctx.push_bound(Type::Int);
    let well_formed = expect_type(
        ctx,
        &motive.open(&Term::Free(hole)),
        &Type::Prop,
        Mode::Logical,
    );
    ctx.truncate(scope);
    well_formed?;
    expect_type(ctx, target, &Type::Int, Mode::Logical)?;
    proof_of(ctx, base, &motive.open(&Term::int(0)))?;
    let nonneg = |n: &Term| Term::int_le(Term::int(0), n.clone());
    check_arm_with(
        ctx,
        step,
        1,
        |_, _| Type::Int,
        |vars| {
            let n = Term::Free(vars[0]);
            vec![nonneg(&n), motive.open(&n)]
        },
        |vars| motive.open(&Term::int_add(Term::Free(vars[0]), Term::int(1))),
    )?;
    // Only the non-negative integers are reached from 0 by successors.
    Ok(Term::implies(nonneg(target), motive.open(target)))
}

/// The induction schema for finite, strictly positive logical data.
#[inline(never)]
fn claim_of_data_induction(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::DataInduction {
        target,
        motives,
        arms,
    } = proof
    else {
        unreachable!()
    };
    let Type::Enum(target_id) = term_type(ctx, target, Mode::Logical)? else {
        return Err(KernelError::InvalidInduction(
            "target must have a logical enum type",
        ));
    };
    let definitions = ctx.definitions().clone();
    let group = definitions
        .logical_enum_group(target_id)
        .ok_or(KernelError::InvalidInduction(
            "target is not a logical enum",
        ))?;
    if motives.len() != group.len()
        || !motives
            .iter()
            .zip(&group)
            .all(|((id, _), expected)| id == expected)
    {
        return Err(KernelError::InvalidInduction(
            "one motive per group member is required, in declaration order",
        ));
    }
    for (id, motive) in motives {
        let scope = ctx.len();
        let var = ctx.push_bound(Type::Enum(*id));
        let checked = expect_type(
            ctx,
            &motive.open(&Term::Free(var)),
            &Type::Prop,
            Mode::Logical,
        );
        ctx.truncate(scope);
        checked?;
    }
    let expected_arms: usize = group
        .iter()
        .map(|id| definitions.enum_variants(*id).unwrap().len())
        .sum();
    expect_arm_count(arms, expected_arms)?;
    let mut arm_index = 0;
    for (id, motive) in motives {
        for (variant, payload) in definitions.enum_variants(*id).unwrap().iter().enumerate() {
            let arm = &arms[arm_index];
            arm_index += 1;
            check_arm_with(
                ctx,
                arm,
                payload.len(),
                |index, vars| field_type(payload, index, |j| Term::Free(vars[j])),
                |vars| {
                    payload
                        .iter()
                        .enumerate()
                        .filter_map(|(index, field)| {
                            let Type::Enum(child) = field else {
                                return None;
                            };
                            motives
                                .iter()
                                .find(|(id, _)| id == child)
                                .map(|(_, motive)| motive.open(&Term::Free(vars[index])))
                        })
                        .collect()
                },
                |vars| {
                    motive.open(&constructor_of_vars(
                        &Type::Enum(*id),
                        variant,
                        vars,
                        payload,
                    ))
                },
            )?;
        }
    }
    let motive = &motives.iter().find(|(id, _)| *id == target_id).unwrap().1;
    Ok(motive.open(target))
}

#[inline(never)]
fn claim_of_prop_induction(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::PropInduction {
        scrutinee,
        motive,
        arms,
    } = proof
    else {
        unreachable!()
    };
    let claim = proof_claim(ctx, scrutinee)?;
    let Term::PropApp(id, arguments) = &claim else {
        return Err(KernelError::InvalidInduction(
            "scrutinee must prove an inductive predicate",
        ));
    };
    let definitions = ctx.definitions().clone();
    let decl = definitions.prop(*id).ok_or(KernelError::UnknownProp)?;
    if !decl.inductive {
        return Err(KernelError::InvalidInduction(
            "predicate is not declared inductive",
        ));
    }
    if motive.binders as usize != decl.params.len() {
        return Err(KernelError::InvalidInduction(
            "motive must bind every header parameter exactly once",
        ));
    }
    let scope = ctx.len();
    let vars: Vec<_> = decl
        .params
        .iter()
        .map(|ty| ctx.push_bound(ty.clone()))
        .collect();
    let checked = expect_type(
        ctx,
        &motive.body.instantiate(vars.len(), |i| Term::Free(vars[i])),
        &Type::Prop,
        Mode::Logical,
    );
    ctx.truncate(scope);
    checked?;
    expect_arm_count(arms, decl.variants.len())?;
    for (arm, variant) in arms.iter().zip(&decl.variants) {
        let telescope = &variant.telescope;
        let body_index = telescope.len() - 1;
        check_arm_with(
            ctx,
            arm,
            telescope.len(),
            |i, vars| field_type(telescope, i, |j| Term::Free(vars[j])),
            |vars| {
                let Type::Proof(body) = field_type(telescope, body_index, |j| Term::Free(vars[j]))
                else {
                    unreachable!("inductive arms end in evidence")
                };
                vec![super::recursive::strengthen_body(
                    &definitions,
                    *id,
                    &body,
                    motive,
                )]
            },
            |vars| {
                motive
                    .body
                    .instantiate(decl.params.len(), |i| Term::Free(vars[i]))
            },
        )?;
    }
    Ok(motive
        .body
        .instantiate(arguments.len(), |i| arguments[i].clone()))
}

/// Accepts `proof` as a proof of `expected`, which must be a proposition.
pub(super) fn proof_of(
    ctx: &mut Context,
    proof: &Proof,
    expected: &Term,
) -> Result<(), KernelError> {
    expect_type(ctx, expected, &Type::Prop, Mode::Logical)?;
    let found = proof_claim(ctx, proof)?;
    if same(&found, expected) {
        Ok(())
    } else {
        Err(KernelError::ProofMismatch {
            expected: Box::new(expected.clone()),
            found: Box::new(found),
        })
    }
}

// --- Public entry points ------------------------------------------------------
//
// Each measures its input before any recursive function sees it.

/// Checks that a type is well formed in the context.
pub fn check_type(ctx: &mut Context, ty: &Type) -> Result<(), KernelError> {
    check_depth([ty.into()])?;
    type_ok(ctx, ty)
}

/// Infers the type of a term, rejecting ill-formed terms.
pub fn infer_term(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    check_depth([term.into()])?;
    term_type(ctx, term, mode)
}

/// Reads off the proposition a proof proves, checking every step.
pub fn infer_proof(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    check_depth([proof.into()])?;
    proof_claim(ctx, proof)
}

/// Accepts `proof` as a proof of `expected`, which must be a proposition.
pub fn check_proof(ctx: &mut Context, proof: &Proof, expected: &Term) -> Result<(), KernelError> {
    check_depth([proof.into(), expected.into()])?;
    proof_of(ctx, proof, expected)
}

/// Checks a call against a function type that is not a declared kernel
/// function: the arguments against the parameter telescope, by the field
/// rule, and returns the result type instantiated at them. The exec checker
/// uses this for calls to ordinary functions.
pub fn check_call(
    ctx: &mut Context,
    signature: &Type,
    arguments: &[Term],
    mode: Mode,
) -> Result<Type, KernelError> {
    check_depth(std::iter::once(signature.into()).chain(arguments.iter().map(Into::into)))?;
    type_ok(ctx, signature)?;
    let Type::Fn(params, result) = signature else {
        return Err(KernelError::NotAFunction(signature.clone()));
    };
    check_fields(ctx, params, arguments, mode)?;
    let mut telescope = params.clone();
    telescope.push((**result).clone());
    Ok(field_type(&telescope, arguments.len(), |j| {
        arguments[j].clone()
    }))
}

/// Checks values against a tuple telescope by the field rule, without
/// building the tuple: a loop's initial state and `continue` arguments.
pub fn check_values(
    ctx: &mut Context,
    telescope: &Type,
    values: &[Term],
    mode: Mode,
) -> Result<(), KernelError> {
    check_depth(std::iter::once(telescope.into()).chain(values.iter().map(Into::into)))?;
    type_ok(ctx, telescope)?;
    let Type::Tuple(fields) = telescope else {
        return Err(KernelError::NotAProduct(telescope.clone()));
    };
    check_fields(ctx, fields, values, mode)
}

/// The type of entry `index` of a telescope with the earlier entries
/// replaced by `earlier`. For a function type, entry `params.len()` is the
/// result type.
pub fn telescope_entry(telescope: &Type, index: usize, earlier: &[Term]) -> Option<Type> {
    let entries: Vec<Type> = match telescope {
        Type::Tuple(fields) => fields.clone(),
        Type::Fn(params, result) => {
            let mut entries = params.clone();
            entries.push((**result).clone());
            entries
        }
        _ => return None,
    };
    (index < entries.len() && earlier.len() >= index)
        .then(|| field_type(&entries, index, |j| earlier[j].clone()))
}

/// The payload telescopes of a type that supports case analysis, and the
/// term for one of its variants applied to payload variables, as it appears
/// in an arm's fact.
pub fn case_variants(ctx: &Context, ty: &Type) -> Option<Vec<Vec<Type>>> {
    data_variants(ctx, ty)
}

pub fn variant_term(ty: &Type, index: usize, vars: &[VarId], payload: &[Type]) -> Term {
    constructor_of_vars(ty, index, vars, payload)
}
