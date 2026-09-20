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
use super::derive::{Chain, fold_claim, symm_at, unfold_claim};
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
    /// `succ(a) + b == succ(a + b)`
    pub nat_succ_add: FnId,
    /// `k == 0 || exists j { k == succ(j) }`
    pub nat_zero_or_succ: FnId,
    /// `nat_le(a, b) => nat_le(succ(a), succ(b))`
    pub nat_le_succ_succ: FnId,
    /// `u8_le(i, limit) => (i == limit => False) => u8_lt(i, limit)`
    pub u8_lt_of_le_of_ne: FnId,
    /// `u8_lt(i, limit) => u8_le(i.wrapping_add(1), limit)`
    pub u8_succ_le_of_lt: FnId,
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
                symm_at(&Type::Nat, &model(&zero_byte), literal),
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

    // succ(a) + b == succ(a + b), by induction on b.
    let succ_add_claim =
        |a: &Term, b: &Term| nat_eq(add(&Term::succ(a.clone()), b), Term::succ(add(a, b)));
    let nat_succ_add = definitions.declare_fn(
        &signature(Type::Nat, 2, vec![], |p| succ_add_claim(&p[0], &p[1])),
        |p| {
            let (a, b) = (p[0].clone(), p[1].clone());
            let sa = Term::succ(a.clone());
            let base = Chain::new(Type::Nat, add(&sa, &zero))
                .step(Proof::Axiom(Axiom::NatAddZero(sa.clone())))
                .rewrite_rev(
                    &Type::Nat,
                    Term::succ,
                    &add(&a, &zero),
                    Proof::Axiom(Axiom::NatAddZero(a.clone())),
                )
                .finish();
            let step = |n: Term, ih: Proof| {
                Chain::new(Type::Nat, add(&sa, &Term::succ(n.clone())))
                    .step(Proof::Axiom(Axiom::NatAddSucc(sa.clone(), n.clone())))
                    .rewrite(Term::succ, ih)
                    .rewrite_rev(
                        &Type::Nat,
                        Term::succ,
                        &add(&a, &Term::succ(n.clone())),
                        Proof::Axiom(Axiom::NatAddSucc(a.clone(), n)),
                    )
                    .finish()
            };
            Term::proof(Proof::nat_induction(
                |n| succ_add_claim(&a, &n),
                base,
                step,
                b,
            ))
        },
    )?;

    // Every natural is zero or a successor, by induction (the hypothesis is
    // not needed). This is case analysis on Nat, which has no term-level
    // case of its own.
    let is_zero = |k: &Term| nat_eq(k.clone(), Term::nat(0));
    let is_succ = |k: &Term| {
        let k = k.clone();
        Term::exists(Type::Nat, |j| nat_eq(k, Term::succ(j)))
    };
    let zero_or_succ = move |k: &Term| prelude.or_prop(is_zero(k), is_succ(k));
    let nat_zero_or_succ = definitions.declare_fn(
        &signature(Type::Nat, 1, vec![], |p| zero_or_succ(&p[0])),
        |p| {
            let side = |k: &Term, variant: usize, proof: Proof| Proof::Construct {
                prop: prelude.or,
                variant,
                params: vec![is_zero(k), is_succ(k)],
                payload: vec![Term::proof(proof)],
            };
            let base = side(&zero, 0, Proof::Refl(zero.clone()));
            let step = |n: Term, _: Proof| {
                let next = Term::succ(n.clone());
                let witnessed = Proof::ExistsIntro {
                    prop: is_succ(&next),
                    witness: n,
                    proof: Box::new(Proof::Refl(next.clone())),
                };
                side(&next, 1, witnessed)
            };
            Term::proof(Proof::nat_induction(
                |k| zero_or_succ(&k),
                base,
                step,
                p[0].clone(),
            ))
        },
    )?;

    // From a + k == b: succ(a) + k == succ(a + k) == succ(b).
    let nat_le_succ_succ = definitions.declare_fn(
        &signature(
            Type::Nat,
            2,
            vec![Box::new(move |p| le(&p[0], &p[1]))],
            |p| le(&Term::succ(p[0].clone()), &Term::succ(p[1].clone())),
        ),
        |p| {
            let (a, b) = (p[0].clone(), p[1].clone());
            let given = unfold_claim(&le(&a, &b), Proof::OfTerm(p[2].clone()));
            let (sa, sb) = (Term::succ(a.clone()), Term::succ(b.clone()));
            let goal = le(&sa, &sb);
            Term::proof(Proof::ExistsElim {
                exists: Box::new(given),
                goal,
                arm: Proof::arm(1, 1, |ks, facts| {
                    let k = ks[0].clone();
                    let chain = Chain::new(Type::Nat, add(&sa, &k))
                        .step(lemma(nat_succ_add, vec![a.clone(), k.clone()]))
                        .rewrite(Term::succ, facts[0].clone())
                        .finish();
                    le_intro(&sa, &sb, k, chain)
                }),
            })
        },
    )?;

    let lt = move |a: &Term, b: &Term| prelude.nat_lt_prop(a.clone(), b.clone());
    let u8_lt = move |a: &Term, b: &Term| prelude.u8_lt_prop(a.clone(), b.clone());

    // i <= limit and i != limit give i < limit. The witness k of i <= limit
    // is zero or a successor. Zero makes the models equal, hence the bytes,
    // which is refuted. A successor succ(j) gives succ(to_nat(i)) + j.
    let u8_lt_of_le_of_ne = definitions.declare_fn(
        &signature(
            Type::U8,
            2,
            vec![
                Box::new(move |p| u8_le(&p[0], &p[1])),
                Box::new(move |p| {
                    Term::implies(
                        Term::eq(Type::U8, p[0].clone(), p[1].clone()),
                        prelude.falsehood_prop(),
                    )
                }),
            ],
            |p| u8_lt(&p[0], &p[1]),
        ),
        |p| {
            let (i, limit) = (p[0].clone(), p[1].clone());
            let (ti, tl) = (model(&i), model(&limit));
            let differs = Proof::OfTerm(p[3].clone());
            let in_model = unfold_claim(&u8_le(&i, &limit), Proof::OfTerm(p[2].clone()));
            let witnessed = unfold_claim(&le(&ti, &tl), in_model);
            let goal = u8_lt(&i, &limit);
            let inner_goal = goal.clone();
            Term::proof(Proof::ExistsElim {
                exists: Box::new(witnessed),
                goal,
                arm: Proof::arm(1, 1, |ks, facts| {
                    let (k, sum) = (ks[0].clone(), facts[0].clone());
                    let when_zero = |k_is_zero: Proof| {
                        // ti + 0 == tl, so ti == tl, so i == limit.
                        let at_zero = Proof::transport(
                            k_is_zero,
                            |hole| nat_eq(add(&ti, &hole), tl.clone()),
                            sum.clone(),
                        );
                        let models_equal = Chain::new(Type::Nat, ti.clone())
                            .step_rev(
                                &add(&ti, &zero),
                                Proof::Axiom(Axiom::NatAddZero(ti.clone())),
                            )
                            .step(at_zero)
                            .finish();
                        let bytes_equal = Chain::new(Type::U8, i.clone())
                            .step_rev(
                                &Term::of_nat(ti.clone()),
                                Proof::Axiom(Axiom::OfToNat(i.clone())),
                            )
                            .rewrite(Term::of_nat, models_equal)
                            .step(Proof::Axiom(Axiom::OfToNat(limit.clone())))
                            .finish();
                        Proof::CaseProof {
                            scrutinee: Box::new(Proof::implies_elim(differs.clone(), bytes_equal)),
                            goal: inner_goal.clone(),
                            arms: vec![],
                        }
                    };
                    let when_succ = |k_is_succ: Proof| Proof::ExistsElim {
                        exists: Box::new(k_is_succ),
                        goal: inner_goal.clone(),
                        arm: Proof::arm(1, 1, |js, shape| {
                            let (j, k_is_succ_j) = (js[0].clone(), shape[0].clone());
                            let sti = Term::succ(ti.clone());
                            let chain = Chain::new(Type::Nat, add(&sti, &j))
                                .step(lemma(nat_succ_add, vec![ti.clone(), j.clone()]))
                                .step_rev(
                                    &add(&ti, &Term::succ(j.clone())),
                                    Proof::Axiom(Axiom::NatAddSucc(ti.clone(), j.clone())),
                                )
                                .rewrite_rev(&Type::Nat, |hole| add(&ti, &hole), &k, k_is_succ_j)
                                .step(sum.clone())
                                .finish();
                            let strict = fold_claim(&lt(&ti, &tl), le_intro(&sti, &tl, j, chain));
                            fold_claim(&u8_lt(&i, &limit), strict)
                        }),
                    };
                    Proof::CaseProof {
                        scrutinee: Box::new(lemma(nat_zero_or_succ, vec![k.clone()])),
                        goal: inner_goal.clone(),
                        arms: vec![
                            Proof::arm(1, 0, |payload, _| {
                                when_zero(Proof::OfTerm(payload[0].clone()))
                            }),
                            Proof::arm(1, 0, |payload, _| {
                                when_succ(Proof::OfTerm(payload[0].clone()))
                            }),
                        ],
                    }
                }),
            })
        },
    )?;

    // i < limit gives i.wrapping_add(1) <= limit. Below a byte, the
    // successor does not wrap: to_nat(i.wrapping_add(1)) == succ(to_nat(i)).
    let u8_succ_le_of_lt = definitions.declare_fn(
        &signature(
            Type::U8,
            2,
            vec![Box::new(move |p| u8_lt(&p[0], &p[1]))],
            |p| u8_le(&Term::wrapping_add(p[0].clone(), Term::U8(1)), &p[1]),
        ),
        |p| {
            let (i, limit) = (p[0].clone(), p[1].clone());
            let (ti, tl) = (model(&i), model(&limit));
            let sti = Term::succ(ti.clone());
            let one = Term::U8(1);
            let next = Term::wrapping_add(i.clone(), one.clone());
            let bound = Term::nat(256);

            let strict = unfold_claim(&u8_lt(&i, &limit), Proof::OfTerm(p[2].clone()));
            let succ_le_limit = unfold_claim(&lt(&ti, &tl), strict);

            // succ(ti) < 256, from succ(ti) <= tl < 256.
            let lifted = lemma(
                nat_le_succ_succ,
                vec![sti.clone(), tl.clone(), Term::proof(succ_le_limit.clone())],
            );
            let limit_fits = unfold_claim(
                &lt(&tl, &bound),
                Proof::Axiom(Axiom::ToNatBound(limit.clone())),
            );
            let fits = fold_claim(
                &lt(&sti, &bound),
                lemma(
                    nat_le_trans,
                    vec![
                        Term::succ(sti.clone()),
                        Term::succ(tl.clone()),
                        bound,
                        Term::proof(lifted),
                        Term::proof(limit_fits),
                    ],
                ),
            );

            // to_nat(i + 1) == succ(ti), through the model of wrapping_add.
            let wrap = |n: Term| Term::to_nat(Term::of_nat(n));
            let model_of_next = Chain::new(Type::Nat, Term::to_nat(next.clone()))
                .rewrite(
                    Term::to_nat,
                    Proof::Axiom(Axiom::WrappingAddModel(i.clone(), one.clone())),
                )
                .rewrite(
                    |hole| wrap(add(&ti, &hole)),
                    Proof::Literal(Term::to_nat(one)),
                )
                .rewrite_rev(
                    &Type::Nat,
                    |hole| wrap(add(&ti, &hole)),
                    &Term::succ(zero.clone()),
                    Proof::Literal(Term::succ(zero.clone())),
                )
                .rewrite(
                    wrap,
                    Proof::Axiom(Axiom::NatAddSucc(ti.clone(), zero.clone())),
                )
                .rewrite(
                    |hole| wrap(Term::succ(hole)),
                    Proof::Axiom(Axiom::NatAddZero(ti.clone())),
                )
                .step(Proof::implies_elim(
                    Proof::Axiom(Axiom::ToOfNat(sti.clone())),
                    fits,
                ))
                .finish();

            let at_next = Proof::transport(
                symm_at(&Type::Nat, &Term::to_nat(next.clone()), model_of_next),
                |hole| le(&hole, &tl),
                succ_le_limit,
            );
            Term::proof(fold_claim(&u8_le(&next, &limit), at_next))
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
        nat_succ_add,
        nat_zero_or_succ,
        nat_le_succ_succ,
        u8_lt_of_le_of_ne,
        u8_succ_le_of_lt,
    })
}
