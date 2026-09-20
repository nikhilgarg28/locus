//! The checking judgments. Each arm of `infer_proof` is one rule of
//! `docs/kernel-contract.md`.

use super::context::{Context, Mode};
use super::defs::Prelude;
use super::error::KernelError;
use super::term::{Axiom, HypRef, Prim, Proof, ProofArm, Term, Type, VarId, field_type};

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
        (Term::Proof(_), Term::Proof(_)) => true,
        (Term::Free(l), Term::Free(r)) => l == r,
        (Term::Bound(l), Term::Bound(r)) => l == r,
        (Term::Bool(l), Term::Bool(r)) => l == r,
        (Term::U8(l), Term::U8(r)) => l == r,
        (Term::Nat(l), Term::Nat(r)) => l == r,
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
        // Two unreachable values of one type: the proofs are irrelevant.
        (Term::Absurd(_, lt), Term::Absurd(_, rt)) => same_type(lt, rt),
        _ => false,
    }
}

pub fn same_type(left: &Type, right: &Type) -> bool {
    match (left, right) {
        (Type::Bool, Type::Bool)
        | (Type::U8, Type::U8)
        | (Type::Nat, Type::Nat)
        | (Type::Prop, Type::Prop) => true,
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
pub fn check_type(ctx: &mut Context, ty: &Type) -> Result<(), KernelError> {
    match ty {
        Type::Bool | Type::U8 | Type::Nat | Type::Prop => Ok(()),
        Type::Proof(prop) => expect_type(ctx, prop, &Type::Prop, Mode::Logical),
        Type::Tuple(fields) => check_telescope(ctx, fields),
        Type::Struct(id) => ctx
            .definitions()
            .struct_fields(*id)
            .map(|_| ())
            .ok_or(KernelError::UnknownStruct),
        Type::Enum(id) => ctx
            .definitions()
            .enum_variants(*id)
            .map(|_| ())
            .ok_or(KernelError::UnknownEnum),
        Type::Fn(params, result) => {
            let mut telescope = params.clone();
            telescope.push((**result).clone());
            check_telescope(ctx, &telescope)
        }
    }
}

/// Each field type must be well formed given variables for the earlier ones.
pub(super) fn check_telescope(ctx: &mut Context, fields: &[Type]) -> Result<(), KernelError> {
    let scope = ctx.len();
    let mut earlier = Vec::new();
    let mut result = Ok(());
    for index in 0..fields.len() {
        let ty = field_type(fields, index, |j| Term::Free(earlier[j]));
        result = check_type(ctx, &ty);
        if result.is_err() {
            break;
        }
        earlier.push(ctx.push_bound(ty));
    }
    ctx.truncate(scope);
    result
}

/// Infers the type of a term, rejecting ill-formed terms.
pub fn infer_term(ctx: &mut Context, term: &Term, mode: Mode) -> Result<Type, KernelError> {
    match term {
        Term::Free(id) => {
            let (ty, ghost) = ctx.var(*id).ok_or(KernelError::UnknownVariable(*id))?;
            if mode == Mode::Executable && ghost {
                return Err(KernelError::GhostInExecutable(*id));
            }
            Ok(ty.clone())
        }
        Term::Bound(_) => Err(KernelError::DanglingBound),
        Term::Bool(_) => Ok(Type::Bool),
        Term::U8(_) => Ok(Type::U8),
        Term::Nat(_) => {
            ghost_former(mode, &Type::Nat)?;
            Ok(Type::Nat)
        }
        Term::Prim(prim, arguments) => {
            let (parameters, result) = prim_signature(*prim);
            if arguments.len() != parameters.len() {
                return Err(KernelError::WrongArity {
                    expected: parameters.len(),
                    found: arguments.len(),
                });
            }
            ghost_former(mode, &result)?;
            for (argument, parameter) in arguments.iter().zip(parameters) {
                expect_type(ctx, argument, parameter, mode)?;
            }
            Ok(result)
        }
        Term::Eq(ty, left, right) => {
            ghost_former(mode, &Type::Prop)?;
            check_type(ctx, ty)?;
            if matches!(ty, Type::Proof(_)) {
                return Err(KernelError::EqualityAtProofType(ty.clone()));
            }
            expect_type(ctx, left, ty, Mode::Logical)?;
            expect_type(ctx, right, ty, Mode::Logical)?;
            Ok(Type::Prop)
        }
        Term::Implies(premise, conclusion) => {
            ghost_former(mode, &Type::Prop)?;
            expect_type(ctx, premise, &Type::Prop, Mode::Logical)?;
            expect_type(ctx, conclusion, &Type::Prop, Mode::Logical)?;
            Ok(Type::Prop)
        }
        Term::Forall(ty, body) => {
            ghost_former(mode, &Type::Prop)?;
            check_type(ctx, ty)?;
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
        Term::Tuple(fields, values) => {
            check_telescope(ctx, fields)?;
            check_fields(ctx, fields, values, mode)?;
            Ok(Type::Tuple(fields.clone()))
        }
        Term::Struct(id, values) => {
            let definitions = ctx.definitions();
            let fields = definitions
                .struct_fields(*id)
                .ok_or(KernelError::UnknownStruct)?;
            check_fields(ctx, fields, values, mode)?;
            Ok(Type::Struct(*id))
        }
        Term::Proj(target, index) => {
            let target_type = infer_term(ctx, target, mode)?;
            let definitions = ctx.definitions();
            let fields = match &target_type {
                Type::Tuple(fields) => fields.as_slice(),
                Type::Struct(id) => definitions
                    .struct_fields(*id)
                    .ok_or(KernelError::UnknownStruct)?,
                _ => return Err(KernelError::NotAProduct(target_type)),
            };
            if *index >= fields.len() {
                return Err(KernelError::NoSuchField {
                    index: *index,
                    fields: fields.len(),
                });
            }
            // Earlier fields are named by projecting from the same target.
            let ty = field_type(fields, *index, |j| Term::proj((**target).clone(), j));
            ghost_former(mode, &ty)?;
            Ok(ty)
        }
        Term::Proof(proof) => {
            let ty = Type::proof(infer_proof(ctx, proof)?);
            ghost_former(mode, &ty)?;
            Ok(ty)
        }
        Term::Fn(id) => {
            let definitions = ctx.definitions();
            let decl = definitions
                .function(*id)
                .ok_or(KernelError::UnknownFunction)?;
            let ty = Type::Fn(decl.params.clone(), Box::new(decl.result.clone()));
            ghost_former(mode, &ty)?;
            Ok(ty)
        }
        Term::Call(callee, arguments) => {
            let callee_type = infer_term(ctx, callee, mode)?;
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
        Term::Variant(id, index, payload) => {
            let definitions = ctx.definitions();
            let variants = definitions
                .enum_variants(*id)
                .ok_or(KernelError::UnknownEnum)?;
            let fields = variants.get(*index).ok_or(KernelError::NoSuchVariant {
                index: *index,
                variants: variants.len(),
            })?;
            check_fields(ctx, fields, payload, mode)?;
            Ok(Type::Enum(*id))
        }
        Term::Case {
            scrutinee,
            result,
            arms,
        } => {
            let scrutinee_type = infer_term(ctx, scrutinee, mode)?;
            let variants = data_variants(ctx, &scrutinee_type)
                .ok_or_else(|| KernelError::NotCaseable((**scrutinee).clone()))?;
            check_type(ctx, result)?;
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
            for (arm, payload) in arms.iter().zip(&variants) {
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
                    let ghost = mode == Mode::Logical || ty.is_ghost();
                    vars.push(ctx.push_local(ty, ghost));
                }
                let body = arm.body.instantiate(vars.len(), |j| Term::Free(vars[j]));
                let checked = expect_type(ctx, &body, result, mode);
                ctx.truncate(scope);
                checked?;
            }
            Ok(result.clone())
        }
        Term::PropApp(id, arguments) => {
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
        Term::Exists(ty, body) => {
            ghost_former(mode, &Type::Prop)?;
            check_type(ctx, ty)?;
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
        Term::Absurd(proof, ty) => {
            let prop = infer_proof(ctx, proof)?;
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
            check_type(ctx, ty)?;
            ghost_former(mode, ty)?;
            Ok(ty.clone())
        }
    }
}

/// The payload telescopes of a data type that supports case analysis:
/// `bool`, with the variants `false` and `true`, or a declared enum.
fn data_variants(ctx: &Context, ty: &Type) -> Option<Vec<Vec<Type>>> {
    match ty {
        Type::Bool => Some(vec![Vec::new(), Vec::new()]),
        Type::Enum(id) => ctx
            .definitions()
            .enum_variants(*id)
            .map(<[Vec<Type>]>::to_vec),
        _ => None,
    }
}

/// Variant `index` of a case-able data type, applied to a payload.
fn constructor(ty: &Type, index: usize, payload: Vec<Term>) -> Term {
    match ty {
        Type::Enum(id) => Term::Variant(*id, index, payload),
        _ => Term::Bool(index == 1),
    }
}

/// The variant index and payload of a term that is literally a constructor.
fn known_constructor(term: &Term) -> Option<(usize, &[Term])> {
    match term {
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
        check_proof(ctx, &arm.body.open_arm(&vars, &hyps), &goal)
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

fn prim_signature(prim: Prim) -> (&'static [Type], Type) {
    match prim {
        Prim::WrappingAdd | Prim::WrappingSub => (&[Type::U8, Type::U8], Type::U8),
        Prim::U8Eq | Prim::U8Lt | Prim::U8Le => (&[Type::U8, Type::U8], Type::Bool),
        Prim::ToNat => (&[Type::U8], Type::Nat),
        Prim::OfNat => (&[Type::Nat], Type::U8),
        Prim::Succ => (&[Type::Nat], Type::Nat),
        Prim::NatAdd => (&[Type::Nat, Type::Nat], Type::Nat),
    }
}

/// Native evaluation of a primitive applied to literals. This is the
/// implementation that must agree with the `u8` model.
fn evaluate(prim: Prim, arguments: &[Term]) -> Option<Term> {
    Some(match (prim, arguments) {
        (Prim::WrappingAdd, [Term::U8(a), Term::U8(b)]) => Term::U8(a.wrapping_add(*b)),
        (Prim::WrappingSub, [Term::U8(a), Term::U8(b)]) => Term::U8(a.wrapping_sub(*b)),
        (Prim::U8Eq, [Term::U8(a), Term::U8(b)]) => Term::Bool(a == b),
        (Prim::U8Lt, [Term::U8(a), Term::U8(b)]) => Term::Bool(a < b),
        (Prim::U8Le, [Term::U8(a), Term::U8(b)]) => Term::Bool(a <= b),
        (Prim::ToNat, [Term::U8(a)]) => Term::Nat(u64::from(*a)),
        (Prim::OfNat, [Term::Nat(n)]) => Term::U8((n % 256) as u8),
        (Prim::Succ, [Term::Nat(n)]) => Term::Nat(n.checked_add(1)?),
        (Prim::NatAdd, [Term::Nat(a), Term::Nat(b)]) => Term::Nat(a.checked_add(*b)?),
        _ => return None,
    })
}

/// The proposition an axiom states, after typing its arguments.
fn axiom_statement(ctx: &mut Context, axiom: &Axiom) -> Result<Term, KernelError> {
    let prelude = ctx.definitions().prelude().ok_or(KernelError::NoPrelude)?;
    let expected = match axiom {
        Axiom::NatAddZero(_)
        | Axiom::NatAddSucc(..)
        | Axiom::NatSuccInjective(..)
        | Axiom::NatSuccNotZero(_)
        | Axiom::ToOfNat(_)
        | Axiom::OfNatWrap(_) => Some(Type::Nat),
        Axiom::ToNatBound(_)
        | Axiom::OfToNat(_)
        | Axiom::WrappingAddModel(..)
        | Axiom::WrappingSubModel(..) => Some(Type::U8),
        Axiom::Reflect(..) => None,
    };
    if let Some(expected) = &expected {
        for term in axiom.terms() {
            expect_type(ctx, term, expected, Mode::Logical)?;
        }
    }
    let nat_eq = |left: Term, right: Term| Term::eq(Type::Nat, left, right);
    let u8_eq = |left: Term, right: Term| Term::eq(Type::U8, left, right);
    let bound = Term::Nat(256);
    Ok(match axiom.clone() {
        Axiom::NatAddZero(a) => nat_eq(Term::nat_add(a.clone(), Term::Nat(0)), a),
        Axiom::NatAddSucc(a, b) => nat_eq(
            Term::nat_add(a.clone(), Term::succ(b.clone())),
            Term::succ(Term::nat_add(a, b)),
        ),
        Axiom::NatSuccInjective(a, b) => Term::implies(
            nat_eq(Term::succ(a.clone()), Term::succ(b.clone())),
            nat_eq(a, b),
        ),
        Axiom::NatSuccNotZero(a) => prelude.not_prop(nat_eq(Term::succ(a), Term::Nat(0))),
        Axiom::ToNatBound(x) => prelude.nat_lt_prop(Term::to_nat(x), bound),
        Axiom::OfToNat(x) => u8_eq(Term::of_nat(Term::to_nat(x.clone())), x),
        Axiom::ToOfNat(n) => Term::implies(
            prelude.nat_lt_prop(n.clone(), bound),
            nat_eq(Term::to_nat(Term::of_nat(n.clone())), n),
        ),
        Axiom::OfNatWrap(n) => u8_eq(
            Term::of_nat(Term::nat_add(n.clone(), bound)),
            Term::of_nat(n),
        ),
        Axiom::WrappingAddModel(a, b) => u8_eq(
            Term::wrapping_add(a.clone(), b.clone()),
            Term::of_nat(Term::nat_add(Term::to_nat(a), Term::to_nat(b))),
        ),
        Axiom::WrappingSubModel(a, b) => u8_eq(
            Term::wrapping_add(Term::wrapping_sub(a.clone(), b.clone()), b),
            a,
        ),
        Axiom::Reflect(comparison, flag) => {
            expect_type(ctx, &comparison, &Type::Bool, Mode::Logical)?;
            let claim = comparison_claim(&prelude, &comparison)
                .ok_or_else(|| KernelError::NoComputationStep(comparison.clone()))?;
            let observed = Term::eq(Type::Bool, comparison, Term::Bool(flag));
            if flag {
                Term::implies(observed, claim)
            } else {
                Term::implies(observed, prelude.not_prop(claim))
            }
        }
    })
}

/// The proposition a runtime comparison decides.
fn comparison_claim(prelude: &Prelude, comparison: &Term) -> Option<Term> {
    let Term::Prim(prim, arguments) = comparison else {
        return None;
    };
    let [left, right] = arguments.as_slice() else {
        return None;
    };
    let (left, right) = (left.clone(), right.clone());
    match prim {
        Prim::U8Eq => Some(Term::eq(Type::U8, left, right)),
        Prim::U8Lt => Some(prelude.u8_lt_prop(left, right)),
        Prim::U8Le => Some(prelude.u8_le_prop(left, right)),
        _ => None,
    }
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
                check_proof(ctx, proof, prop)?;
            }
            _ if expected.is_ghost() => expect_type(ctx, value, &expected, Mode::Logical)?,
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
    let found = infer_term(ctx, term, mode)?;
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
pub fn infer_proof(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    match proof {
        Proof::Hyp(HypRef::Free(id)) => ctx
            .hyp(*id)
            .cloned()
            .ok_or(KernelError::UnknownHypothesis(*id)),
        Proof::Hyp(HypRef::Bound(_)) => Err(KernelError::DanglingBound),
        Proof::OfTerm(term) => match infer_term(ctx, term, Mode::Logical)? {
            Type::Proof(prop) => Ok(*prop),
            other => Err(KernelError::NotAProofType(other)),
        },
        Proof::Refl(term) => {
            let ty = infer_term(ctx, term, Mode::Logical)?;
            if matches!(ty, Type::Proof(_)) {
                return Err(KernelError::EqualityAtProofType(ty));
            }
            Ok(Term::eq(ty, term.clone(), term.clone()))
        }
        Proof::Transport {
            eq,
            template,
            proof,
        } => {
            let equality = infer_proof(ctx, eq)?;
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
            check_proof(ctx, proof, &template.open(&left))?;
            Ok(template.open(&right))
        }
        Proof::ImpliesIntro { hyp, body } => {
            expect_type(ctx, hyp, &Type::Prop, Mode::Logical)?;
            let scope = ctx.len();
            let id = ctx.push_hyp(hyp.clone());
            let conclusion = infer_proof(ctx, &body.open_hyp(id));
            ctx.truncate(scope);
            Ok(Term::implies(hyp.clone(), conclusion?))
        }
        Proof::ImpliesElim(implication, premise) => {
            let prop = infer_proof(ctx, implication)?;
            let Term::Implies(expected, conclusion) = prop else {
                return Err(KernelError::NotAnImplication(prop));
            };
            check_proof(ctx, premise, &expected)?;
            Ok(*conclusion)
        }
        Proof::ForallIntro { ty, body } => {
            check_type(ctx, ty)?;
            let scope = ctx.len();
            let var = ctx.push_bound(ty.clone());
            let instance = infer_proof(ctx, &body.open_var(&Term::Free(var)));
            ctx.truncate(scope);
            Ok(Term::Forall(ty.clone(), Box::new(instance?.close(var))))
        }
        Proof::ForallElim(universal, argument) => {
            let prop = infer_proof(ctx, universal)?;
            let Term::Forall(ty, body) = prop else {
                return Err(KernelError::NotUniversal(prop));
            };
            expect_type(ctx, argument, &ty, Mode::Logical)?;
            Ok(body.open(argument))
        }
        Proof::Projection(term) => {
            let Term::Proj(target, index) = term else {
                return Err(KernelError::NoComputationStep(term.clone()));
            };
            let (Term::Tuple(_, values) | Term::Struct(_, values)) = &**target else {
                return Err(KernelError::NoComputationStep(term.clone()));
            };
            let ty = infer_term(ctx, term, Mode::Logical)?;
            if matches!(ty, Type::Proof(_)) {
                return Err(KernelError::EqualityAtProofType(ty));
            }
            // The projection names earlier fields as projections, the value
            // names them directly; the step is offered only when the two
            // types already coincide.
            let value = &values[*index];
            if !same_type(&infer_term(ctx, value, Mode::Logical)?, &ty) {
                return Err(KernelError::NoComputationStep(term.clone()));
            }
            Ok(Term::eq(ty, term.clone(), value.clone()))
        }
        Proof::Literal(term) => {
            let Term::Prim(prim, arguments) = term else {
                return Err(KernelError::NoComputationStep(term.clone()));
            };
            let value = evaluate(*prim, arguments)
                .ok_or_else(|| KernelError::NoComputationStep(term.clone()))?;
            let ty = infer_term(ctx, term, Mode::Logical)?;
            Ok(Term::eq(ty, term.clone(), value))
        }
        Proof::Definition(term) => {
            let Term::Call(callee, arguments) = term else {
                return Err(KernelError::NoComputationStep(term.clone()));
            };
            let Term::Fn(id) = &**callee else {
                return Err(KernelError::NoComputationStep(term.clone()));
            };
            let ty = infer_term(ctx, term, Mode::Logical)?;
            if matches!(ty, Type::Proof(_)) {
                return Err(KernelError::EqualityAtProofType(ty));
            }
            let definitions = ctx.definitions();
            let decl = definitions
                .function(*id)
                .ok_or(KernelError::UnknownFunction)?;
            let unfolded = decl
                .body
                .instantiate(arguments.len(), |j| arguments[j].clone());
            Ok(Term::eq(ty, term.clone(), unfolded))
        }
        Proof::CaseStep(term) => {
            let Term::Case {
                scrutinee, arms, ..
            } = term
            else {
                return Err(KernelError::NoComputationStep(term.clone()));
            };
            let Some((index, payload)) = known_constructor(scrutinee) else {
                return Err(KernelError::NoComputationStep(term.clone()));
            };
            let ty = infer_term(ctx, term, Mode::Logical)?;
            let chosen = arms[index]
                .body
                .instantiate(payload.len(), |j| payload[j].clone());
            Ok(Term::eq(ty, term.clone(), chosen))
        }
        Proof::Construct {
            prop,
            variant,
            params,
            payload,
        } => {
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
        Proof::CaseProof {
            scrutinee,
            goal,
            arms,
        } => {
            let proved = infer_proof(ctx, scrutinee)?;
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
                        |index, vars| {
                            field_type(&variant.telescope, index, |j| Term::Free(vars[j]))
                        },
                        |vars| {
                            let stated = variant
                                .conclusion
                                .iter()
                                .map(|term| term.instantiate(vars.len(), |j| Term::Free(vars[j])));
                            decl.params
                                .iter()
                                .zip(arguments)
                                .zip(stated)
                                .map(|((ty, actual), stated)| {
                                    Term::eq(ty.clone(), actual.clone(), stated)
                                })
                                .collect()
                        },
                        goal,
                    )?;
                }
            }
            Ok(goal.clone())
        }
        Proof::CaseData {
            scrutinee,
            goal,
            arms,
        } => {
            let ty = infer_term(ctx, scrutinee, Mode::Logical)?;
            let variants = data_variants(ctx, &ty)
                .ok_or_else(|| KernelError::NotCaseable(scrutinee.clone()))?;
            expect_type(ctx, goal, &Type::Prop, Mode::Logical)?;
            expect_arm_count(arms, variants.len())?;
            for (index, (arm, payload)) in arms.iter().zip(&variants).enumerate() {
                check_arm(
                    ctx,
                    arm,
                    payload.len(),
                    |field, vars| field_type(payload, field, |j| Term::Free(vars[j])),
                    |vars| {
                        let built = vars.iter().copied().map(Term::Free).collect();
                        vec![Term::eq(
                            ty.clone(),
                            scrutinee.clone(),
                            constructor(&ty, index, built),
                        )]
                    },
                    goal,
                )?;
            }
            Ok(goal.clone())
        }
        Proof::ExistsIntro {
            prop,
            witness,
            proof,
        } => {
            expect_type(ctx, prop, &Type::Prop, Mode::Logical)?;
            let Term::Exists(ty, body) = prop else {
                return Err(KernelError::NotExistential(prop.clone()));
            };
            expect_type(ctx, witness, ty, Mode::Logical)?;
            check_proof(ctx, proof, &body.open(witness))?;
            Ok(prop.clone())
        }
        Proof::ExistsElim { exists, goal, arm } => {
            let proved = infer_proof(ctx, exists)?;
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
        Proof::ExcludedMiddle(prop) => {
            let prelude = ctx.definitions().prelude().ok_or(KernelError::NoPrelude)?;
            expect_type(ctx, prop, &Type::Prop, Mode::Logical)?;
            Ok(prelude.or_prop(prop.clone(), prelude.not_prop(prop.clone())))
        }
        Proof::Axiom(axiom) => axiom_statement(ctx, axiom),
        Proof::NatInduction {
            motive,
            base,
            step,
            target,
        } => {
            let scope = ctx.len();
            let hole = ctx.push_bound(Type::Nat);
            let well_formed = expect_type(
                ctx,
                &motive.open(&Term::Free(hole)),
                &Type::Prop,
                Mode::Logical,
            );
            ctx.truncate(scope);
            well_formed?;
            expect_type(ctx, target, &Type::Nat, Mode::Logical)?;
            check_proof(ctx, base, &motive.open(&Term::Nat(0)))?;
            check_arm_with(
                ctx,
                step,
                1,
                |_, _| Type::Nat,
                |vars| vec![motive.open(&Term::Free(vars[0]))],
                |vars| motive.open(&Term::succ(Term::Free(vars[0]))),
            )?;
            Ok(motive.open(target))
        }
    }
}

/// Accepts `proof` as a proof of `expected`, which must be a proposition.
pub fn check_proof(ctx: &mut Context, proof: &Proof, expected: &Term) -> Result<(), KernelError> {
    expect_type(ctx, expected, &Type::Prop, Mode::Logical)?;
    let found = infer_proof(ctx, proof)?;
    if same(&found, expected) {
        Ok(())
    } else {
        Err(KernelError::ProofMismatch {
            expected: Box::new(expected.clone()),
            found: Box::new(found),
        })
    }
}
