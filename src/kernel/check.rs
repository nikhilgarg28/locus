//! The checking judgments. Each arm of `infer_proof` is one rule of
//! `docs/kernel-contract.md`.

use super::context::{Context, Mode};
use super::error::KernelError;
use super::term::{HypRef, Prim, Proof, Term, Type, field_type};

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
        (Term::Prim(lp, la), Term::Prim(rp, ra)) => lp == rp && all(la, ra),
        (Term::Eq(lt, ll, lr), Term::Eq(rt, rl, rr)) => {
            same_type(lt, rt) && same(ll, rl) && same(lr, rr)
        }
        (Term::Implies(lp, lc), Term::Implies(rp, rc)) => same(lp, rp) && same(lc, rc),
        (Term::Forall(lt, lb), Term::Forall(rt, rb)) => same_type(lt, rt) && same(lb, rb),
        (Term::Tuple(lf, lv), Term::Tuple(rf, rv)) => same_types(lf, rf) && all(lv, rv),
        (Term::Struct(li, lv), Term::Struct(ri, rv)) => li == ri && all(lv, rv),
        (Term::Proj(lt, li), Term::Proj(rt, ri)) => li == ri && same(lt, rt),
        _ => false,
    }
}

pub fn same_type(left: &Type, right: &Type) -> bool {
    match (left, right) {
        (Type::Bool, Type::Bool) | (Type::U8, Type::U8) | (Type::Prop, Type::Prop) => true,
        (Type::Proof(l), Type::Proof(r)) => same(l, r),
        (Type::Tuple(l), Type::Tuple(r)) => same_types(l, r),
        (Type::Struct(l), Type::Struct(r)) => l == r,
        _ => false,
    }
}

fn same_types(left: &[Type], right: &[Type]) -> bool {
    left.len() == right.len() && left.iter().zip(right).all(|(l, r)| same_type(l, r))
}

/// Checks that a type is well formed in the context.
pub fn check_type(ctx: &mut Context, ty: &Type) -> Result<(), KernelError> {
    match ty {
        Type::Bool | Type::U8 | Type::Prop => Ok(()),
        Type::Proof(prop) => expect_type(ctx, prop, &Type::Prop, Mode::Logical),
        Type::Tuple(fields) => check_telescope(ctx, fields),
        Type::Struct(id) => ctx
            .definitions()
            .struct_fields(*id)
            .map(|_| ())
            .ok_or(KernelError::UnknownStruct),
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
        Term::Prim(Prim::WrappingAdd | Prim::WrappingSub, arguments) => {
            if arguments.len() != 2 {
                return Err(KernelError::WrongArity {
                    expected: 2,
                    found: arguments.len(),
                });
            }
            for argument in arguments {
                expect_type(ctx, argument, &Type::U8, mode)?;
            }
            Ok(Type::U8)
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

fn expect_type(
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
            let [Term::U8(left), Term::U8(right)] = arguments.as_slice() else {
                return Err(KernelError::NoComputationStep(term.clone()));
            };
            let value = match prim {
                Prim::WrappingAdd => left.wrapping_add(*right),
                Prim::WrappingSub => left.wrapping_sub(*right),
            };
            Ok(Term::eq(Type::U8, term.clone(), Term::U8(value)))
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
            expected: expected.clone(),
            found,
        })
    }
}
