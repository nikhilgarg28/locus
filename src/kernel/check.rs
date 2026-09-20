//! The checking judgments. Each arm of `infer_proof` is one rule of
//! `docs/kernel-contract.md`.

use super::context::{Context, Mode};
use super::error::KernelError;
use super::term::{HypRef, Prim, Proof, Term, Type};

/// The kernel's only comparison of terms. Locally nameless binding makes
/// renaming of bound variables structural equality. Proof irrelevance is
/// vacuous until terms can contain proofs (gate K2).
pub fn same(left: &Term, right: &Term) -> bool {
    left == right
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
            proposition_former(mode)?;
            expect_type(ctx, left, ty, Mode::Logical)?;
            expect_type(ctx, right, ty, Mode::Logical)?;
            Ok(Type::Prop)
        }
        Term::Implies(premise, conclusion) => {
            proposition_former(mode)?;
            expect_type(ctx, premise, &Type::Prop, Mode::Logical)?;
            expect_type(ctx, conclusion, &Type::Prop, Mode::Logical)?;
            Ok(Type::Prop)
        }
        Term::Forall(ty, body) => {
            proposition_former(mode)?;
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
    }
}

/// A proposition has no runtime value, so forming one is never executable.
fn proposition_former(mode: Mode) -> Result<(), KernelError> {
    match mode {
        Mode::Logical => Ok(()),
        Mode::Executable => Err(KernelError::GhostTypeInExecutable(Type::Prop)),
    }
}

fn expect_type(
    ctx: &mut Context,
    term: &Term,
    expected: &Type,
    mode: Mode,
) -> Result<(), KernelError> {
    let found = infer_term(ctx, term, mode)?;
    if &found == expected {
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
        Proof::Refl(term) => {
            let ty = infer_term(ctx, term, Mode::Logical)?;
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
