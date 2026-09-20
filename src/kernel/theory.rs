//! The beginning of the kernel-level prelude: lemmas about the internal
//! `Nat` and the `u8` model, proved from the axioms and checked by the
//! kernel like any other declaration. Nothing here is trusted. If a proof
//! below were wrong, `declare` would fail.
//!
//! These are written in the kernel's own term language because the source
//! language cannot name `Nat`. The orderings on `u8` are defined through
//! `to_nat`, so facts about them come from facts about `Nat`, by reasoning
//! and induction, never by enumerating bytes.

use super::defs::{Definitions, Prelude};
use super::derive::{Chain, fold_claim, unfold_claim};
use super::error::KernelError;
use super::term::{Axiom, FnId, Proof, Term, Type};

/// Identities of the declared lemmas.
#[derive(Clone, Copy, Debug)]
pub struct Theory {
    /// `(a + b) + c == a + (b + c)`
    pub nat_add_assoc: FnId,
    /// `0 + a == a`
    pub nat_zero_add: FnId,
    /// `nat_le(a, a)`
    pub nat_le_refl: FnId,
    /// `nat_le(0, a)`
    pub nat_zero_le: FnId,
    /// `nat_le(a, b) => nat_le(b, c) => nat_le(a, c)`, as proof parameters
    pub nat_le_trans: FnId,
    /// `u8_le(a, a)`
    pub u8_le_refl: FnId,
    /// `u8_le(0, x)`
    pub u8_zero_le: FnId,
    /// `u8_le(a, b) => u8_le(b, c) => u8_le(a, c)`, as proof parameters
    pub u8_le_trans: FnId,
}

fn nat_eq(left: Term, right: Term) -> Term {
    Term::eq(Type::Nat, left, right)
}

fn add(left: &Term, right: &Term) -> Term {
    Term::nat_add(left.clone(), right.clone())
}

fn lemma(id: FnId, arguments: Vec<Term>) -> Proof {
    Proof::OfTerm(Term::call(Term::Fn(id), arguments))
}

/// A premise of a lemma, as a claim about the parameters before it.
type Premise = Box<dyn Fn(&[Term]) -> Term>;

/// A signature of `count` parameters of type `over`, followed by the given
/// proof parameters, returning a proof of `claim`.
fn signature(
    over: Type,
    count: usize,
    premises: Vec<Premise>,
    claim: impl Fn(&[Term]) -> Term,
) -> Type {
    Type::function(count + premises.len(), |params| {
        if params.len() < count {
            over.clone()
        } else if params.len() < count + premises.len() {
            Type::proof(premises[params.len() - count](params))
        } else {
            Type::proof(claim(params))
        }
    })
}

pub fn declare(definitions: &mut Definitions, prelude: &Prelude) -> Result<Theory, KernelError> {
    let prelude = *prelude;
    let zero = Term::nat(0);

    // (a + b) + c == a + (b + c), by induction on c.
    let assoc_claim = |a: &Term, b: &Term, c: &Term| nat_eq(add(&add(a, b), c), add(a, &add(b, c)));
    let nat_add_assoc = definitions.declare_fn(
        &signature(Type::Nat, 3, vec![], |p| assoc_claim(&p[0], &p[1], &p[2])),
        |p| {
            let (a, b, c) = (p[0].clone(), p[1].clone(), p[2].clone());
            let ab = add(&a, &b);
            let base = Chain::new(Type::Nat, add(&ab, &zero))
                .step(Proof::Axiom(Axiom::NatAddZero(ab.clone())))
                .rewrite_rev(
                    &Type::Nat,
                    |hole| add(&a, &hole),
                    &add(&b, &zero),
                    Proof::Axiom(Axiom::NatAddZero(b.clone())),
                )
                .finish();
            let step = |n: Term, ih: Proof| {
                let bn = add(&b, &n);
                Chain::new(Type::Nat, add(&ab, &Term::succ(n.clone())))
                    .step(Proof::Axiom(Axiom::NatAddSucc(ab.clone(), n.clone())))
                    .rewrite(Term::succ, ih)
                    .step_rev(
                        &add(&a, &Term::succ(bn.clone())),
                        Proof::Axiom(Axiom::NatAddSucc(a.clone(), bn)),
                    )
                    .rewrite_rev(
                        &Type::Nat,
                        |hole| add(&a, &hole),
                        &add(&b, &Term::succ(n.clone())),
                        Proof::Axiom(Axiom::NatAddSucc(b.clone(), n)),
                    )
                    .finish()
            };
            Term::proof(Proof::nat_induction(
                |n| assoc_claim(&a, &b, &n),
                base,
                step,
                c,
            ))
        },
    )?;

    // 0 + a == a, by induction on a.
    let zero_add_claim = |a: &Term| nat_eq(add(&Term::nat(0), a), a.clone());
    let nat_zero_add = definitions.declare_fn(
        &signature(Type::Nat, 1, vec![], |p| zero_add_claim(&p[0])),
        |p| {
            let base = Proof::Axiom(Axiom::NatAddZero(zero.clone()));
            let step = |n: Term, ih: Proof| {
                Chain::new(Type::Nat, add(&zero, &Term::succ(n.clone())))
                    .step(Proof::Axiom(Axiom::NatAddSucc(zero.clone(), n)))
                    .rewrite(Term::succ, ih)
                    .finish()
            };
            Term::proof(Proof::nat_induction(
                |n| zero_add_claim(&n),
                base,
                step,
                p[0].clone(),
            ))
        },
    )?;

    // nat_le(a, b) unfolds to: exists k { a + k == b }.
    let le = move |a: &Term, b: &Term| prelude.nat_le_prop(a.clone(), b.clone());
    let le_body = |a: &Term, b: &Term| {
        let (a, b) = (a.clone(), b.clone());
        Term::exists(Type::Nat, |k| nat_eq(Term::nat_add(a, k), b))
    };
    let le_intro = |a: &Term, b: &Term, witness: Term, proof: Proof| {
        fold_claim(
            &le(a, b),
            Proof::ExistsIntro {
                prop: le_body(a, b),
                witness,
                proof: Box::new(proof),
            },
        )
    };

    let nat_le_refl = definitions.declare_fn(
        &signature(Type::Nat, 1, vec![], |p| le(&p[0], &p[0])),
        |p| {
            let a = &p[0];
            let proof = Proof::Axiom(Axiom::NatAddZero(a.clone()));
            Term::proof(le_intro(a, a, zero.clone(), proof))
        },
    )?;

    let nat_zero_le = definitions.declare_fn(
        &signature(Type::Nat, 1, vec![], |p| le(&Term::nat(0), &p[0])),
        |p| {
            let a = &p[0];
            let proof = lemma(nat_zero_add, vec![a.clone()]);
            Term::proof(le_intro(&zero, a, a.clone(), proof))
        },
    )?;

    // From a + k == b and b + m == c: a + (k + m) == (a + k) + m == b + m == c.
    let nat_le_trans = definitions.declare_fn(
        &signature(
            Type::Nat,
            3,
            vec![
                Box::new(move |p| le(&p[0], &p[1])),
                Box::new(move |p| le(&p[1], &p[2])),
            ],
            |p| le(&p[0], &p[2]),
        ),
        |p| {
            let (a, b, c) = (p[0].clone(), p[1].clone(), p[2].clone());
            let first = unfold_claim(&le(&a, &b), Proof::OfTerm(p[3].clone()));
            let second = unfold_claim(&le(&b, &c), Proof::OfTerm(p[4].clone()));
            let goal = le(&a, &c);
            let inner_goal = goal.clone();
            Term::proof(Proof::ExistsElim {
                exists: Box::new(first),
                goal,
                arm: Proof::arm(1, 1, |ks, a_plus_k| {
                    let (k, a_plus_k) = (ks[0].clone(), a_plus_k[0].clone());
                    Proof::ExistsElim {
                        exists: Box::new(second),
                        goal: inner_goal,
                        arm: Proof::arm(1, 1, |ms, b_plus_m| {
                            let (m, b_plus_m) = (ms[0].clone(), b_plus_m[0].clone());
                            let km = add(&k, &m);
                            let chain = Chain::new(Type::Nat, add(&a, &km))
                                .step_rev(
                                    &add(&add(&a, &k), &m),
                                    lemma(nat_add_assoc, vec![a.clone(), k.clone(), m.clone()]),
                                )
                                .rewrite(|hole| add(&hole, &m), a_plus_k)
                                .step(b_plus_m)
                                .finish();
                            le_intro(&a, &c, km, chain)
                        }),
                    }
                }),
            })
        },
    )?;

    // The u8 orderings are the Nat orderings of the models.
    let u8_le = move |a: &Term, b: &Term| prelude.u8_le_prop(a.clone(), b.clone());
    let model = |x: &Term| Term::to_nat(x.clone());

    let u8_le_refl = definitions.declare_fn(
        &signature(Type::U8, 1, vec![], |p| u8_le(&p[0], &p[0])),
        |p| {
            let x = &p[0];
            Term::proof(fold_claim(&u8_le(x, x), lemma(nat_le_refl, vec![model(x)])))
        },
    )?;

    let u8_zero_le = definitions.declare_fn(
        &signature(Type::U8, 1, vec![], |p| u8_le(&Term::U8(0), &p[0])),
        |p| {
            let x = &p[0];
            // nat_le(0, to_nat(x)), with the 0 rewritten to to_nat(0u8).
            let zero_byte = Term::U8(0);
            let from_model = lemma(nat_zero_le, vec![model(x)]);
            let literal = Proof::Literal(model(&zero_byte));
            let model_x = model(x);
            let rewritten = Proof::transport(
                super::derive::symm_at(&Type::Nat, &model(&zero_byte), literal),
                |hole| le(&hole, &model_x),
                from_model,
            );
            Term::proof(fold_claim(&u8_le(&zero_byte, x), rewritten))
        },
    )?;

    let u8_le_trans = definitions.declare_fn(
        &signature(
            Type::U8,
            3,
            vec![
                Box::new(move |p| u8_le(&p[0], &p[1])),
                Box::new(move |p| u8_le(&p[1], &p[2])),
            ],
            |p| u8_le(&p[0], &p[2]),
        ),
        |p| {
            let (a, b, c) = (&p[0], &p[1], &p[2]);
            let first = unfold_claim(&u8_le(a, b), Proof::OfTerm(p[3].clone()));
            let second = unfold_claim(&u8_le(b, c), Proof::OfTerm(p[4].clone()));
            let in_model = lemma(
                nat_le_trans,
                vec![
                    model(a),
                    model(b),
                    model(c),
                    Term::proof(first),
                    Term::proof(second),
                ],
            );
            Term::proof(fold_claim(&u8_le(a, c), in_model))
        },
    )?;

    Ok(Theory {
        nat_add_assoc,
        nat_zero_add,
        nat_le_refl,
        nat_zero_le,
        nat_le_trans,
        u8_le_refl,
        u8_zero_le,
        u8_le_trans,
    })
}
