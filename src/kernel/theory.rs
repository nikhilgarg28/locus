//! The beginning of the kernel-level prelude: lemmas about the internal
//! `Nat` and the `u8` model, about `Int`, and about every machine integer
//! type over its model in `Int`, proved from the axioms and checked by the
//! kernel like any other declaration. Nothing here is trusted. If a proof
//! below were wrong, `declare` would fail.
//!
//! These are written in the kernel's own term language because the source
//! language cannot name `Nat`. The orderings on `u8` are defined through
//! `to_nat`, so facts about them come from facts about `Nat`, by reasoning
//! and induction, never by enumerating bytes. The lemmas about a machine
//! type `T` are stated over the views, `int_le(view[T](a), view[T](b))`,
//! and are one family declared once at each type; `Theory::lemma_names`
//! lists them under the names the source language will call them by.

use super::defs::{Definitions, Prelude};
use super::derive::{Chain, fold_claim, symm_at, unfold_claim};
use super::error::KernelError;
use super::machine::MachineInt;
use super::term::{Axiom, CmpOp, FnId, Proof, Term, Type};

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
    /// `a + b == b + a`
    pub nat_add_comm: FnId,
    /// `a + b == a + c => b == c`
    pub nat_add_cancel_left: FnId,
    /// `nat_lt(a, b) || nat_le(b, a)`
    pub nat_lt_or_le: FnId,
    /// `u8_le(b, a) => to_nat(a.wrapping_sub(b)) + to_nat(b) == to_nat(a)`:
    /// a difference that does not wrap
    pub u8_sub_model: FnId,
    /// `u8_le(b, a) => u8_le(a.wrapping_sub(b), a)`
    pub u8_sub_le: FnId,
    /// `u8_le(b, c) => u8_le(c, a) => u8_le(a.wrapping_sub(c), a.wrapping_sub(b))`
    pub u8_sub_le_sub: FnId,
    /// `a == b => b == a`, at `u8`
    pub u8_eq_symm: FnId,
    // --- Int, and the machine types over their views. Below, `a < b` is
    // `int_lt(a, b)`, that is `a + 1 <= b`, and `!P` is `P => False`.
    /// `a < b => a <= b`
    pub int_le_of_lt: FnId,
    /// `a <= b => !(a == b) => a < b`
    pub int_lt_of_le_of_ne: FnId,
    /// `a <= b => c + a <= c + b`
    pub int_le_add_left: FnId,
    /// `a <= b => a + c <= b + c`
    pub int_le_add_right: FnId,
    /// `a <= b => a - c <= b - c`
    pub int_le_sub: FnId,
    /// `a <= b => 0 <= c => a * c <= b * c`
    pub int_mul_le_mul_nonneg: FnId,
    /// The family of lemmas about each machine type, in the order of
    /// `MachineInt::ALL`; `Theory::machine` looks one up.
    pub machine: [MachineLemmas; 8],
}

/// Declares `MachineLemmas`, the lemmas about one machine type `T`, and
/// `MachineLemmas::names`, which names each at a type as `u16_le_trans`.
/// The names are the source names: `<type>_<lemma>`.
macro_rules! machine_lemmas {
    ($($(#[$doc:meta])* $field:ident),* $(,)?) => {
        /// The lemmas about one machine type `T`, over the views: `v(x)`
        /// stands for `view[T](x)`, `a < b` for `int_lt(a, b)`, `!P` for
        /// `P => False`, and `le[T]`, `lt[T]`, `eq[T]` for the runtime
        /// comparisons `Prim::Cmp`. Each premise is a proof parameter.
        #[derive(Clone, Copy, Debug)]
        pub struct MachineLemmas {
            $($(#[$doc])* pub $field: FnId,)*
        }

        impl MachineLemmas {
            /// The lemmas under their names at the given type.
            pub fn names(&self, ty: MachineInt) -> Vec<(&'static str, FnId)> {
                match ty {
                    MachineInt::U8 => vec![$((concat!("u8_", stringify!($field)), self.$field),)*],
                    MachineInt::U16 => vec![$((concat!("u16_", stringify!($field)), self.$field),)*],
                    MachineInt::U32 => vec![$((concat!("u32_", stringify!($field)), self.$field),)*],
                    MachineInt::U64 => vec![$((concat!("u64_", stringify!($field)), self.$field),)*],
                    MachineInt::I8 => vec![$((concat!("i8_", stringify!($field)), self.$field),)*],
                    MachineInt::I16 => vec![$((concat!("i16_", stringify!($field)), self.$field),)*],
                    MachineInt::I32 => vec![$((concat!("i32_", stringify!($field)), self.$field),)*],
                    MachineInt::I64 => vec![$((concat!("i64_", stringify!($field)), self.$field),)*],
                }
            }
        }
    };
}

machine_lemmas! {
    /// `v(a) <= v(a)`
    le_refl,
    /// `v(a) <= v(b) => v(b) <= v(c) => v(a) <= v(c)`
    le_trans,
    /// `v(a) < v(b) => v(a) <= v(b)`
    le_of_lt,
    /// `v(a) <= v(b) => !(v(a) == v(b)) => v(a) < v(b)`
    lt_of_le_of_ne,
    /// `!(v(a) < v(a))`
    lt_irrefl,
    /// `v(a) <= v(b) => v(b) <= v(a) => a ==[T] b`
    le_antisymm,
    /// `v(a) == v(b) => a ==[T] b`
    view_injective,
    /// `And(min(T) <= v(a), v(a) <= max(T))`
    view_bounds,
    /// `le[T](a, b) == true => v(a) <= v(b)`
    le_of_cmp,
    /// `v(a) <= v(b) => le[T](a, b) == true`
    cmp_of_le,
    /// `lt[T](a, b) == true => v(a) < v(b)`
    lt_of_cmp,
    /// `v(a) < v(b) => lt[T](a, b) == true`
    cmp_of_lt,
    /// `eq[T](a, b) == true => a ==[T] b`
    eq_of_cmp,
    /// `a ==[T] b => eq[T](a, b) == true`
    cmp_of_eq,
    /// `le[T](a, b) == false => v(b) < v(a)`
    lt_of_not_le,
    /// `lt[T](a, b) == false => v(b) <= v(a)`
    le_of_not_lt,
}

impl Theory {
    /// The lemmas about a machine type.
    pub fn machine(&self, ty: MachineInt) -> &MachineLemmas {
        let index = MachineInt::ALL
            .iter()
            .position(|other| *other == ty)
            .expect("every machine type is in the table");
        &self.machine[index]
    }

    /// The lemmas about `Int` and about the machine types under the names
    /// the source language calls them by: `int_le_of_lt`, `u16_le_trans`.
    /// The lemmas about the `u8` model over `Nat` are not here; the
    /// elaborator names those itself until E5, when this table replaces
    /// them. The names are distinct, and a test lists them so that a
    /// rename is deliberate.
    pub fn lemma_names(&self) -> Vec<(&'static str, FnId)> {
        let mut names = vec![
            ("int_le_of_lt", self.int_le_of_lt),
            ("int_lt_of_le_of_ne", self.int_lt_of_le_of_ne),
            ("int_le_add_left", self.int_le_add_left),
            ("int_le_add_right", self.int_le_add_right),
            ("int_le_sub", self.int_le_sub),
            ("int_mul_le_mul_nonneg", self.int_mul_le_mul_nonneg),
        ];
        for ty in MachineInt::ALL {
            names.extend(self.machine(ty).names(ty));
        }
        names
    }
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

    // --- Lemmas the explicit evidence of the examples is written with ------------

    // a + b == b + a, by induction on b.
    let comm_claim = |a: &Term, b: &Term| nat_eq(add(a, b), add(b, a));
    let nat_add_comm = definitions.declare_fn(
        &signature(Type::Nat, 2, vec![], |p| comm_claim(&p[0], &p[1])),
        |p| {
            let (a, b) = (p[0].clone(), p[1].clone());
            let base = Chain::new(Type::Nat, add(&a, &zero))
                .step(Proof::Axiom(Axiom::NatAddZero(a.clone())))
                .step_rev(&add(&zero, &a), lemma(nat_zero_add, vec![a.clone()]))
                .finish();
            let step = |n: Term, ih: Proof| {
                Chain::new(Type::Nat, add(&a, &Term::succ(n.clone())))
                    .step(Proof::Axiom(Axiom::NatAddSucc(a.clone(), n.clone())))
                    .rewrite(Term::succ, ih)
                    .step_rev(
                        &add(&Term::succ(n.clone()), &a),
                        lemma(nat_succ_add, vec![n, a.clone()]),
                    )
                    .finish()
            };
            Term::proof(Proof::nat_induction(|n| comm_claim(&a, &n), base, step, b))
        },
    )?;

    // a + b == a + c gives b == c, by induction on a, through the
    // injectivity of succ.
    let cancel_claim = |a: &Term, b: &Term, c: &Term| {
        Term::implies(nat_eq(add(a, b), add(a, c)), nat_eq(b.clone(), c.clone()))
    };
    let nat_add_cancel_left = definitions.declare_fn(
        &signature(
            Type::Nat,
            3,
            vec![Box::new(|p| nat_eq(add(&p[0], &p[1]), add(&p[0], &p[2])))],
            |p| nat_eq(p[1].clone(), p[2].clone()),
        ),
        |p| {
            let (a, b, c) = (p[0].clone(), p[1].clone(), p[2].clone());
            let base = Proof::implies_intro(nat_eq(add(&zero, &b), add(&zero, &c)), |given| {
                Chain::new(Type::Nat, b.clone())
                    .step_rev(&add(&zero, &b), lemma(nat_zero_add, vec![b.clone()]))
                    .step(given)
                    .step(lemma(nat_zero_add, vec![c.clone()]))
                    .finish()
            });
            let step = |n: Term, ih: Proof| {
                let sn = Term::succ(n.clone());
                Proof::implies_intro(nat_eq(add(&sn, &b), add(&sn, &c)), |given| {
                    let lifted = Chain::new(Type::Nat, Term::succ(add(&n, &b)))
                        .step_rev(
                            &add(&sn, &b),
                            lemma(nat_succ_add, vec![n.clone(), b.clone()]),
                        )
                        .step(given)
                        .step(lemma(nat_succ_add, vec![n.clone(), c.clone()]))
                        .finish();
                    let inner = Proof::implies_elim(
                        Proof::Axiom(Axiom::NatSuccInjective(add(&n, &b), add(&n, &c))),
                        lifted,
                    );
                    Proof::implies_elim(ih, inner)
                })
            };
            Term::proof(Proof::implies_elim(
                Proof::nat_induction(|n| cancel_claim(&n, &b, &c), base, step, a),
                Proof::OfTerm(p[3].clone()),
            ))
        },
    )?;

    // nat_lt(a, b) || nat_le(b, a): the order is total. By induction on a,
    // for every b, with b zero or a successor in each case.
    let lt_or_le = move |a: &Term, b: &Term| prelude.or_prop(lt(a, b), le(b, a));
    let nat_lt_or_le = definitions.declare_fn(
        &signature(Type::Nat, 2, vec![], |p| lt_or_le(&p[0], &p[1])),
        |p| {
            let (a, b) = (p[0].clone(), p[1].clone());
            let side = |x: &Term, y: &Term, variant: usize, proof: Proof| Proof::Construct {
                prop: prelude.or,
                variant,
                params: vec![lt(x, y), le(y, x)],
                payload: vec![Term::proof(proof)],
            };
            let base = Proof::forall_intro(Type::Nat, |y| {
                let goal = lt_or_le(&zero, &y);
                by_zero_or_succ(
                    nat_zero_or_succ,
                    &y,
                    &goal,
                    |y_is_zero| {
                        // le(y, 0), from le(0, 0).
                        let at = Proof::transport(
                            symm_at(&Type::Nat, &y, y_is_zero),
                            |hole| le(&hole, &zero),
                            lemma(nat_le_refl, vec![zero.clone()]),
                        );
                        side(&zero, &y, 1, at)
                    },
                    |j, y_is_succ_j| {
                        // lt(0, y): le(succ 0, succ j), from le(0, j).
                        let below = lemma(
                            nat_le_succ_succ,
                            vec![
                                zero.clone(),
                                j.clone(),
                                Term::proof(lemma(nat_zero_le, vec![j.clone()])),
                            ],
                        );
                        let at = Proof::transport(
                            symm_at(&Type::Nat, &y, y_is_succ_j),
                            |hole| le(&Term::succ(zero.clone()), &hole),
                            below,
                        );
                        side(&zero, &y, 0, fold_claim(&lt(&zero, &y), at))
                    },
                )
            });
            let step = |n: Term, ih: Proof| {
                let sn = Term::succ(n.clone());
                Proof::forall_intro(Type::Nat, |y| {
                    let goal = lt_or_le(&sn, &y);
                    by_zero_or_succ(
                        nat_zero_or_succ,
                        &y,
                        &goal,
                        |y_is_zero| {
                            let at = Proof::transport(
                                symm_at(&Type::Nat, &y, y_is_zero),
                                |hole| le(&hole, &sn),
                                lemma(nat_zero_le, vec![sn.clone()]),
                            );
                            side(&sn, &y, 1, at)
                        },
                        |j, y_is_succ_j| {
                            let sj = Term::succ(j.clone());
                            let succ_j_is_y = symm_at(&Type::Nat, &y, y_is_succ_j);
                            Proof::CaseProof {
                                scrutinee: Box::new(Proof::forall_elim(ih, j.clone())),
                                goal: goal.clone(),
                                arms: vec![
                                    Proof::arm(1, 0, |payload, _| {
                                        // lt(n, j) lifts to lt(succ n, succ j).
                                        let opened = unfold_claim(
                                            &lt(&n, &j),
                                            Proof::OfTerm(payload[0].clone()),
                                        );
                                        let lifted = lemma(
                                            nat_le_succ_succ,
                                            vec![sn.clone(), j.clone(), Term::proof(opened)],
                                        );
                                        let strict = fold_claim(&lt(&sn, &sj), lifted);
                                        let at = Proof::transport(
                                            succ_j_is_y.clone(),
                                            |hole| lt(&sn, &hole),
                                            strict,
                                        );
                                        side(&sn, &y, 0, at)
                                    }),
                                    Proof::arm(1, 0, |payload, _| {
                                        // le(j, n) lifts to le(succ j, succ n).
                                        let lifted = lemma(
                                            nat_le_succ_succ,
                                            vec![
                                                j.clone(),
                                                n.clone(),
                                                Term::proof(Proof::OfTerm(payload[0].clone())),
                                            ],
                                        );
                                        let at = Proof::transport(
                                            succ_j_is_y.clone(),
                                            |hole| le(&hole, &sn),
                                            lifted,
                                        );
                                        side(&sn, &y, 1, at)
                                    }),
                                ],
                            }
                        },
                    )
                })
            };
            let general = Proof::nat_induction(
                |n| Term::forall(Type::Nat, |y| lt_or_le(&n, &y)),
                base,
                step,
                a,
            );
            Term::proof(Proof::forall_elim(general, b))
        },
    )?;

    // With b <= a, the difference a.wrapping_sub(b) does not wrap:
    // to_nat(a.wrapping_sub(b)) + to_nat(b) == to_nat(a). The model of the
    // sum is either below 256, and then it is what of_nat gives back, or
    // not, and then b <= a is contradicted.
    let sub_model_claim = |a: &Term, b: &Term| {
        nat_eq(
            add(&model(&Term::wrapping_sub(a.clone(), b.clone())), &model(b)),
            model(a),
        )
    };
    let u8_sub_model = definitions.declare_fn(
        &signature(
            Type::U8,
            2,
            vec![Box::new(move |p| u8_le(&p[1], &p[0]))],
            |p| sub_model_claim(&p[0], &p[1]),
        ),
        |p| {
            let (a, b) = (p[0].clone(), p[1].clone());
            let d = Term::wrapping_sub(a.clone(), b.clone());
            let (ta, tb, td) = (model(&a), model(&b), model(&d));
            let sum = add(&td, &tb);
            let bound = Term::nat(256);
            let goal = nat_eq(sum.clone(), ta.clone());
            // of_nat(td + tb) == a
            let wrapped = Chain::new(Type::U8, Term::of_nat(sum.clone()))
                .step_rev(
                    &Term::wrapping_add(d.clone(), b.clone()),
                    Proof::Axiom(Axiom::WrappingAddModel(d.clone(), b.clone())),
                )
                .step(Proof::Axiom(Axiom::WrappingSubModel(a.clone(), b.clone())))
                .finish();
            // The bound on a byte's model, as a witness: succ(to_nat(x)) + j == 256.
            let bound_of = |x: &Term| {
                unfold_claim(
                    &le(&Term::succ(model(x)), &bound),
                    unfold_claim(
                        &lt(&model(x), &bound),
                        Proof::Axiom(Axiom::ToNatBound(x.clone())),
                    ),
                )
            };
            let below = |strict: Proof| {
                let back = Proof::implies_elim(Proof::Axiom(Axiom::ToOfNat(sum.clone())), strict);
                Chain::new(Type::Nat, sum.clone())
                    .step_rev(&Term::to_nat(Term::of_nat(sum.clone())), back)
                    .rewrite(Term::to_nat, wrapped.clone())
                    .finish()
            };
            let above = |at_least: Proof| {
                let witnessed = unfold_claim(&le(&bound, &sum), at_least);
                Proof::ExistsElim {
                    exists: Box::new(witnessed),
                    goal: goal.clone(),
                    arm: Proof::arm(1, 1, |ms, facts| {
                        // wrap: 256 + m == td + tb
                        let (m, wrap) = (ms[0].clone(), facts[0].clone());
                        let same_byte = Chain::new(Type::U8, Term::of_nat(m.clone()))
                            .step_rev(
                                &Term::of_nat(add(&m, &bound)),
                                Proof::Axiom(Axiom::OfNatWrap(m.clone())),
                            )
                            .rewrite(
                                Term::of_nat,
                                lemma(nat_add_comm, vec![m.clone(), bound.clone()]),
                            )
                            .rewrite(Term::of_nat, wrap.clone())
                            .step(wrapped.clone())
                            .finish();
                        let ordered = unfold_claim(
                            &le(&tb, &ta),
                            unfold_claim(&u8_le(&b, &a), Proof::OfTerm(p[2].clone())),
                        );
                        Proof::ExistsElim {
                            exists: Box::new(bound_of(&d)),
                            goal: goal.clone(),
                            arm: Proof::arm(1, 1, |js, facts| {
                                // d_fits: succ(td) + j == 256
                                let (j, d_fits) = (js[0].clone(), facts[0].clone());
                                let jm = add(&j, &m);
                                // td + succ(j + m) == td + tb, so succ(j + m) == tb.
                                let shifted =
                                    Chain::new(Type::Nat, add(&td, &Term::succ(jm.clone())))
                                        .step(Proof::Axiom(Axiom::NatAddSucc(
                                            td.clone(),
                                            jm.clone(),
                                        )))
                                        .step_rev(
                                            &add(&Term::succ(td.clone()), &jm),
                                            lemma(nat_succ_add, vec![td.clone(), jm.clone()]),
                                        )
                                        .step_rev(
                                            &add(&add(&Term::succ(td.clone()), &j), &m),
                                            lemma(
                                                nat_add_assoc,
                                                vec![Term::succ(td.clone()), j.clone(), m.clone()],
                                            ),
                                        )
                                        .rewrite(|hole| add(&hole, &m), d_fits)
                                        .step(wrap.clone())
                                        .finish();
                                let tb_is = lemma(
                                    nat_add_cancel_left,
                                    vec![
                                        td.clone(),
                                        Term::succ(jm.clone()),
                                        tb.clone(),
                                        Term::proof(shifted),
                                    ],
                                );
                                Proof::ExistsElim {
                                    exists: Box::new(bound_of(&b)),
                                    goal: goal.clone(),
                                    arm: Proof::arm(1, 1, |j2s, facts| {
                                        // b_fits: succ(tb) + j2 == 256
                                        let (j2, b_fits) = (j2s[0].clone(), facts[0].clone());
                                        let sm = Term::succ(m.clone());
                                        let sj = Term::succ(j.clone());
                                        // succ m + succ j == succ tb
                                        let two_up = Chain::new(Type::Nat, add(&sm, &sj))
                                            .step(Proof::Axiom(Axiom::NatAddSucc(
                                                sm.clone(),
                                                j.clone(),
                                            )))
                                            .rewrite(
                                                Term::succ,
                                                lemma(nat_succ_add, vec![m.clone(), j.clone()]),
                                            )
                                            .rewrite(
                                                |hole| Term::succ(Term::succ(hole)),
                                                lemma(nat_add_comm, vec![m.clone(), j.clone()]),
                                            )
                                            .rewrite(Term::succ, tb_is.clone())
                                            .finish();
                                        // m < 256: succ m + (succ j + j2) == 256
                                        let m_fits =
                                            Chain::new(Type::Nat, add(&sm, &add(&sj, &j2)))
                                                .step_rev(
                                                    &add(&add(&sm, &sj), &j2),
                                                    lemma(
                                                        nat_add_assoc,
                                                        vec![sm.clone(), sj.clone(), j2.clone()],
                                                    ),
                                                )
                                                .rewrite(|hole| add(&hole, &j2), two_up)
                                                .step(b_fits)
                                                .finish();
                                        let m_below = fold_claim(
                                            &lt(&m, &bound),
                                            le_intro(&sm, &bound, add(&sj, &j2), m_fits),
                                        );
                                        // ta == m
                                        let through = Chain::new(
                                            Type::Nat,
                                            Term::to_nat(Term::of_nat(m.clone())),
                                        )
                                        .rewrite(Term::to_nat, same_byte.clone())
                                        .finish();
                                        let ta_is = Chain::new(Type::Nat, ta.clone())
                                            .step_rev(
                                                &Term::to_nat(Term::of_nat(m.clone())),
                                                through,
                                            )
                                            .step(Proof::implies_elim(
                                                Proof::Axiom(Axiom::ToOfNat(m.clone())),
                                                m_below,
                                            ))
                                            .finish();
                                        Proof::ExistsElim {
                                            exists: Box::new(ordered),
                                            goal: goal.clone(),
                                            arm: Proof::arm(1, 1, |ks, facts| {
                                                // below_a: tb + k == ta
                                                let (k, below_a) =
                                                    (ks[0].clone(), facts[0].clone());
                                                let jk = add(&j, &k);
                                                let turned = Chain::new(Type::Nat, add(&sm, &j))
                                                    .step(lemma(
                                                        nat_succ_add,
                                                        vec![m.clone(), j.clone()],
                                                    ))
                                                    .rewrite(
                                                        Term::succ,
                                                        lemma(
                                                            nat_add_comm,
                                                            vec![m.clone(), j.clone()],
                                                        ),
                                                    )
                                                    .finish();
                                                // m + succ(j + k) == m + 0, so succ(j + k) == 0.
                                                let stuck = Chain::new(
                                                    Type::Nat,
                                                    add(&m, &Term::succ(jk.clone())),
                                                )
                                                .step(Proof::Axiom(Axiom::NatAddSucc(
                                                    m.clone(),
                                                    jk.clone(),
                                                )))
                                                .step_rev(
                                                    &add(&sm, &jk),
                                                    lemma(
                                                        nat_succ_add,
                                                        vec![m.clone(), jk.clone()],
                                                    ),
                                                )
                                                .step_rev(
                                                    &add(&add(&sm, &j), &k),
                                                    lemma(
                                                        nat_add_assoc,
                                                        vec![sm.clone(), j.clone(), k.clone()],
                                                    ),
                                                )
                                                .rewrite(|hole| add(&hole, &k), turned)
                                                .rewrite(|hole| add(&hole, &k), tb_is.clone())
                                                .step(below_a)
                                                .step(ta_is)
                                                .step_rev(
                                                    &add(&m, &zero),
                                                    Proof::Axiom(Axiom::NatAddZero(m.clone())),
                                                )
                                                .finish();
                                                let zero_is_succ = lemma(
                                                    nat_add_cancel_left,
                                                    vec![
                                                        m.clone(),
                                                        Term::succ(jk.clone()),
                                                        zero.clone(),
                                                        Term::proof(stuck),
                                                    ],
                                                );
                                                Proof::CaseProof {
                                                    scrutinee: Box::new(Proof::implies_elim(
                                                        Proof::Axiom(Axiom::NatSuccNotZero(jk)),
                                                        zero_is_succ,
                                                    )),
                                                    goal: goal.clone(),
                                                    arms: vec![],
                                                }
                                            }),
                                        }
                                    }),
                                }
                            }),
                        }
                    }),
                }
            };
            Term::proof(Proof::CaseProof {
                scrutinee: Box::new(lemma(nat_lt_or_le, vec![sum.clone(), bound.clone()])),
                goal: goal.clone(),
                arms: vec![
                    Proof::arm(1, 0, |payload, _| below(Proof::OfTerm(payload[0].clone()))),
                    Proof::arm(1, 0, |payload, _| above(Proof::OfTerm(payload[0].clone()))),
                ],
            })
        },
    )?;

    // b <= a gives a.wrapping_sub(b) <= a: the model of b is the witness.
    let u8_sub_le = definitions.declare_fn(
        &signature(
            Type::U8,
            2,
            vec![Box::new(move |p| u8_le(&p[1], &p[0]))],
            |p| u8_le(&Term::wrapping_sub(p[0].clone(), p[1].clone()), &p[0]),
        ),
        |p| {
            let (a, b) = (p[0].clone(), p[1].clone());
            let d = Term::wrapping_sub(a.clone(), b.clone());
            let sum_is = lemma(
                u8_sub_model,
                vec![
                    a.clone(),
                    b.clone(),
                    Term::proof(Proof::OfTerm(p[2].clone())),
                ],
            );
            Term::proof(fold_claim(
                &u8_le(&d, &a),
                le_intro(&model(&d), &model(&a), model(&b), sum_is),
            ))
        },
    )?;

    // b <= c <= a gives a.wrapping_sub(c) <= a.wrapping_sub(b): subtracting
    // more leaves less. With tb + k == tc, the witness is k.
    let u8_sub_le_sub = definitions.declare_fn(
        &signature(
            Type::U8,
            3,
            vec![
                Box::new(move |p| u8_le(&p[1], &p[2])),
                Box::new(move |p| u8_le(&p[2], &p[0])),
            ],
            |p| {
                u8_le(
                    &Term::wrapping_sub(p[0].clone(), p[2].clone()),
                    &Term::wrapping_sub(p[0].clone(), p[1].clone()),
                )
            },
        ),
        |p| {
            let (a, b, c) = (p[0].clone(), p[1].clone(), p[2].clone());
            let hbc = Term::proof(Proof::OfTerm(p[3].clone()));
            let hca = Term::proof(Proof::OfTerm(p[4].clone()));
            let db = Term::wrapping_sub(a.clone(), b.clone());
            let dc = Term::wrapping_sub(a.clone(), c.clone());
            let (tb, tc, tdb, tdc) = (model(&b), model(&c), model(&db), model(&dc));
            let hba = lemma(
                u8_le_trans,
                vec![b.clone(), c.clone(), a.clone(), hbc.clone(), hca.clone()],
            );
            let via_b = lemma(u8_sub_model, vec![a.clone(), b.clone(), Term::proof(hba)]);
            let via_c = lemma(u8_sub_model, vec![a.clone(), c.clone(), hca]);
            let ordered = unfold_claim(
                &le(&tb, &tc),
                unfold_claim(&u8_le(&b, &c), Proof::OfTerm(p[3].clone())),
            );
            let goal = u8_le(&dc, &db);
            Term::proof(Proof::ExistsElim {
                exists: Box::new(ordered),
                goal: goal.clone(),
                arm: Proof::arm(1, 1, |ks, facts| {
                    // gap: tb + k == tc
                    let (k, gap) = (ks[0].clone(), facts[0].clone());
                    // tb + tdb == tb + (tdc + k)
                    let chain = Chain::new(Type::Nat, add(&tb, &tdb))
                        .step(lemma(nat_add_comm, vec![tb.clone(), tdb.clone()]))
                        .step(via_b)
                        .step_rev(&add(&tdc, &tc), via_c)
                        .rewrite(
                            |hole| add(&tdc, &hole),
                            symm_at(&Type::Nat, &add(&tb, &k), gap),
                        )
                        .step_rev(
                            &add(&add(&tdc, &tb), &k),
                            lemma(nat_add_assoc, vec![tdc.clone(), tb.clone(), k.clone()]),
                        )
                        .rewrite(
                            |hole| add(&hole, &k),
                            lemma(nat_add_comm, vec![tdc.clone(), tb.clone()]),
                        )
                        .step(lemma(
                            nat_add_assoc,
                            vec![tb.clone(), tdc.clone(), k.clone()],
                        ))
                        .finish();
                    let diff = lemma(
                        nat_add_cancel_left,
                        vec![tb.clone(), tdb.clone(), add(&tdc, &k), Term::proof(chain)],
                    );
                    fold_claim(
                        &goal,
                        le_intro(&tdc, &tdb, k, symm_at(&Type::Nat, &tdb, diff)),
                    )
                }),
            })
        },
    )?;

    // a == b gives b == a.
    let u8_eq_symm = definitions.declare_fn(
        &signature(
            Type::U8,
            2,
            vec![Box::new(|p| Term::eq(Type::U8, p[0].clone(), p[1].clone()))],
            |p| Term::eq(Type::U8, p[1].clone(), p[0].clone()),
        ),
        |p| Term::proof(symm_at(&Type::U8, &p[0], Proof::OfTerm(p[2].clone()))),
    )?;

    let (int_lemmas, machine) = declare_int_and_machine(definitions, &prelude)?;
    let [
        int_le_of_lt,
        int_lt_of_le_of_ne,
        int_le_add_left,
        int_le_add_right,
        int_le_sub,
        int_mul_le_mul_nonneg,
    ] = int_lemmas;

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
        nat_add_comm,
        nat_add_cancel_left,
        nat_lt_or_le,
        u8_sub_model,
        u8_sub_le,
        u8_sub_le_sub,
        u8_eq_symm,
        int_le_of_lt,
        int_lt_of_le_of_ne,
        int_le_add_left,
        int_le_add_right,
        int_le_sub,
        int_mul_le_mul_nonneg,
        machine,
    })
}

/// `False`, from a proof of it, at any goal: case analysis with no arms.
fn absurd(refuted: Proof, goal: &Term) -> Proof {
    Proof::CaseProof {
        scrutinee: Box::new(refuted),
        goal: goal.clone(),
        arms: vec![],
    }
}

/// `a <= b` and `!(a == b)` give `a < b`, over `Int`: the order is total,
/// so either `b <= a`, and then antisymmetry makes the two equal, which is
/// refuted, or `a < b`.
fn lt_of_le_of_ne(a: &Term, b: &Term, le_ab: Proof, ne: Proof) -> Proof {
    let goal = Term::int_lt(a.clone(), b.clone());
    let equal = |le_ba: Proof| {
        Proof::implies_elim(
            Proof::implies_elim(
                Proof::Axiom(Axiom::IntLeAntisymm(a.clone(), b.clone())),
                le_ab.clone(),
            ),
            le_ba,
        )
    };
    Proof::CaseProof {
        scrutinee: Box::new(Proof::Axiom(Axiom::IntLeTotal(b.clone(), a.clone()))),
        goal: goal.clone(),
        arms: vec![
            Proof::arm(1, 0, |payload, _| {
                let le_ba = Proof::OfTerm(payload[0].clone());
                absurd(Proof::implies_elim(ne, equal(le_ba)), &goal)
            }),
            Proof::arm(1, 0, |payload, _| Proof::OfTerm(payload[0].clone())),
        ],
    }
}

/// `c == true`, for a comparison `c` at a machine type, from a proof of
/// the proposition it decides: case analysis on the boolean, where the
/// `false` case is refuted through `cmp_reflect`.
fn cmp_true_of(comparison: &Term, proof: Proof) -> Proof {
    let goal = Term::eq(Type::Bool, comparison.clone(), Term::Bool(true));
    Proof::CaseData {
        scrutinee: comparison.clone(),
        goal: goal.clone(),
        arms: vec![
            Proof::arm(0, 1, |_, hyps| {
                let refuting = Proof::implies_elim(
                    Proof::Axiom(Axiom::CmpReflect(comparison.clone(), false)),
                    hyps[0].clone(),
                );
                absurd(Proof::implies_elim(refuting, proof), &goal)
            }),
            Proof::arm(0, 1, |_, hyps| hyps[0].clone()),
        ],
    }
}

/// The lemmas about `Int`, in the order of the fields of `Theory`, and the
/// family of lemmas about each machine type, in the order of
/// `MachineInt::ALL`.
fn declare_int_and_machine(
    definitions: &mut Definitions,
    prelude: &Prelude,
) -> Result<([FnId; 6], [MachineLemmas; 8]), KernelError> {
    let prelude = *prelude;
    let le = |a: &Term, b: &Term| Term::int_le(a.clone(), b.clone());
    let lt = |a: &Term, b: &Term| Term::int_lt(a.clone(), b.clone());
    let int_eq = |a: &Term, b: &Term| Term::eq(Type::Int, a.clone(), b.clone());
    let not = move |p: Term| prelude.not_prop(p);
    let add = |a: &Term, b: &Term| Term::int_add(a.clone(), b.clone());
    let sub = |a: &Term, b: &Term| Term::int_sub(a.clone(), b.clone());
    let mul = |a: &Term, b: &Term| Term::int_mul(a.clone(), b.clone());
    let given = |p: &Term| Proof::OfTerm(p.clone());
    let zero = Term::int(0);

    // --- Int ------------------------------------------------------------------------

    // a < b is a + 1 <= b; a linear certificate closes the gap of one.
    let int_le_of_lt = definitions.declare_fn(
        &signature(
            Type::Int,
            2,
            vec![Box::new(move |p| lt(&p[0], &p[1]))],
            |p| le(&p[0], &p[1]),
        ),
        |p| Term::proof(Proof::linear(le(&p[0], &p[1]), 1, vec![(given(&p[2]), 1)])),
    )?;

    let int_lt_of_le_of_ne = definitions.declare_fn(
        &signature(
            Type::Int,
            2,
            vec![
                Box::new(move |p| le(&p[0], &p[1])),
                Box::new(move |p| not(int_eq(&p[0], &p[1]))),
            ],
            |p| lt(&p[0], &p[1]),
        ),
        |p| Term::proof(lt_of_le_of_ne(&p[0], &p[1], given(&p[2]), given(&p[3]))),
    )?;

    // Adding on the left, on the right, and subtracting: each a linear
    // certificate from the premise alone, as the atoms cancel; on the
    // right it is the axiom itself.
    let int_le_add_left = definitions.declare_fn(
        &signature(
            Type::Int,
            3,
            vec![Box::new(move |p| le(&p[0], &p[1]))],
            |p| le(&add(&p[2], &p[0]), &add(&p[2], &p[1])),
        ),
        |p| {
            Term::proof(Proof::linear(
                le(&add(&p[2], &p[0]), &add(&p[2], &p[1])),
                1,
                vec![(given(&p[3]), 1)],
            ))
        },
    )?;

    let int_le_add_right = definitions.declare_fn(
        &signature(
            Type::Int,
            3,
            vec![Box::new(move |p| le(&p[0], &p[1]))],
            |p| le(&add(&p[0], &p[2]), &add(&p[1], &p[2])),
        ),
        |p| {
            Term::proof(Proof::implies_elim(
                Proof::Axiom(Axiom::IntLeAdd(p[0].clone(), p[1].clone(), p[2].clone())),
                given(&p[3]),
            ))
        },
    )?;

    let int_le_sub = definitions.declare_fn(
        &signature(
            Type::Int,
            3,
            vec![Box::new(move |p| le(&p[0], &p[1]))],
            |p| le(&sub(&p[0], &p[2]), &sub(&p[1], &p[2])),
        ),
        |p| {
            Term::proof(Proof::linear(
                le(&sub(&p[0], &p[2]), &sub(&p[1], &p[2])),
                1,
                vec![(given(&p[3]), 1)],
            ))
        },
    )?;

    // a <= b and 0 <= c give a * c <= b * c. int_le_mul gives
    // 0 <= (b + -a) * c; the ring axioms, read as linear equations in the
    // products, turn that into the goal: the certificate takes the
    // distributions and commutations as its constraints, and a product
    // whose one side is a form with no atoms, c * (a + -a), reads as 0.
    let int_mul_le_mul_nonneg = definitions.declare_fn(
        &signature(
            Type::Int,
            3,
            vec![
                Box::new(move |p| le(&p[0], &p[1])),
                Box::new(move |p| le(&Term::int(0), &p[2])),
            ],
            |p| le(&mul(&p[0], &p[2]), &mul(&p[1], &p[2])),
        ),
        |p| {
            let (a, b, c) = (&p[0], &p[1], &p[2]);
            let neg_a = Term::int_neg(a.clone());
            let d = add(b, &neg_a);
            let nonneg_d = Proof::linear(le(&zero, &d), 1, vec![(given(&p[3]), 1)]);
            let nonneg_product = Proof::implies_elim(
                Proof::implies_elim(
                    Proof::Axiom(Axiom::IntLeMul(d.clone(), c.clone())),
                    nonneg_d,
                ),
                given(&p[4]),
            );
            let axiom = |axiom: Axiom| Proof::Axiom(axiom);
            Term::proof(Proof::linear(
                le(&mul(a, c), &mul(b, c)),
                1,
                vec![
                    (nonneg_product, 1),
                    (axiom(Axiom::IntMulComm(d.clone(), c.clone())), 1),
                    (
                        axiom(Axiom::IntMulAdd(c.clone(), b.clone(), neg_a.clone())),
                        1,
                    ),
                    (axiom(Axiom::IntMulComm(c.clone(), b.clone())), 1),
                    (axiom(Axiom::IntMulComm(c.clone(), a.clone())), -1),
                    (axiom(Axiom::IntMulAdd(c.clone(), a.clone(), neg_a)), -1),
                ],
            ))
        },
    )?;

    // --- Each machine type, over its views ------------------------------------------

    let mut families = Vec::new();
    for ty in MachineInt::ALL {
        let over = Type::machine(ty);
        let v = move |x: &Term| Term::view(ty, x.clone());
        let same = move |a: &Term, b: &Term| Term::eq(Type::machine(ty), a.clone(), b.clone());
        let cmp = move |op: CmpOp, a: &Term, b: &Term| Term::cmp(op, ty, a.clone(), b.clone());
        let is = |c: Term, flag: bool| Term::eq(Type::Bool, c, Term::Bool(flag));
        let view_le = move |p: &[Term], i: usize, j: usize| le(&v(&p[i]), &v(&p[j]));
        let view_lt = move |p: &[Term], i: usize, j: usize| lt(&v(&p[i]), &v(&p[j]));
        let view_eq = move |p: &[Term], i: usize, j: usize| int_eq(&v(&p[i]), &v(&p[j]));

        let le_refl = definitions.declare_fn(
            &signature(over.clone(), 1, vec![], |p| view_le(p, 0, 0)),
            |p| Term::proof(Proof::Axiom(Axiom::IntLeRefl(v(&p[0])))),
        )?;

        let le_trans = definitions.declare_fn(
            &signature(
                over.clone(),
                3,
                vec![
                    Box::new(move |p| view_le(p, 0, 1)),
                    Box::new(move |p| view_le(p, 1, 2)),
                ],
                |p| view_le(p, 0, 2),
            ),
            |p| {
                Term::proof(Proof::implies_elim(
                    Proof::implies_elim(
                        Proof::Axiom(Axiom::IntLeTrans(v(&p[0]), v(&p[1]), v(&p[2]))),
                        given(&p[3]),
                    ),
                    given(&p[4]),
                ))
            },
        )?;

        let le_of_lt = definitions.declare_fn(
            &signature(
                over.clone(),
                2,
                vec![Box::new(move |p| view_lt(p, 0, 1))],
                |p| view_le(p, 0, 1),
            ),
            |p| Term::proof(Proof::linear(view_le(p, 0, 1), 1, vec![(given(&p[2]), 1)])),
        )?;

        let lt_of_le_of_ne = definitions.declare_fn(
            &signature(
                over.clone(),
                2,
                vec![
                    Box::new(move |p| view_le(p, 0, 1)),
                    Box::new(move |p| not(view_eq(p, 0, 1))),
                ],
                |p| view_lt(p, 0, 1),
            ),
            |p| {
                Term::proof(lt_of_le_of_ne(
                    &v(&p[0]),
                    &v(&p[1]),
                    given(&p[2]),
                    given(&p[3]),
                ))
            },
        )?;

        let lt_irrefl = definitions.declare_fn(
            &signature(over.clone(), 1, vec![], |p| not(view_lt(p, 0, 0))),
            |p| Term::proof(Proof::Axiom(Axiom::IntLtIrrefl(v(&p[0])))),
        )?;

        // v(a) == v(b) gives a == b: a is wrap(v(a)), which is wrap(v(b)),
        // which is b, both by wrap_view. This is the injectivity of view,
        // by congruence, so it needs no axiom of its own.
        let view_injective = definitions.declare_fn(
            &signature(
                over.clone(),
                2,
                vec![Box::new(move |p| view_eq(p, 0, 1))],
                |p| same(&p[0], &p[1]),
            ),
            |p| {
                let (a, b) = (&p[0], &p[1]);
                Term::proof(
                    Chain::new(over.clone(), a.clone())
                        .step_rev(
                            &Term::wrap(ty, v(a)),
                            Proof::Axiom(Axiom::WrapView(ty, a.clone())),
                        )
                        .rewrite(|hole| Term::wrap(ty, hole), given(&p[2]))
                        .step(Proof::Axiom(Axiom::WrapView(ty, b.clone())))
                        .finish(),
                )
            },
        )?;

        let le_antisymm = definitions.declare_fn(
            &signature(
                over.clone(),
                2,
                vec![
                    Box::new(move |p| view_le(p, 0, 1)),
                    Box::new(move |p| view_le(p, 1, 0)),
                ],
                |p| same(&p[0], &p[1]),
            ),
            |p| {
                let views_equal = Proof::implies_elim(
                    Proof::implies_elim(
                        Proof::Axiom(Axiom::IntLeAntisymm(v(&p[0]), v(&p[1]))),
                        given(&p[2]),
                    ),
                    given(&p[3]),
                );
                Term::proof(lemma(
                    view_injective,
                    vec![p[0].clone(), p[1].clone(), Term::proof(views_equal)],
                ))
            },
        )?;

        let lower = move |x: &Term| le(&Term::Int(ty.min()), &v(x));
        let upper = move |x: &Term| le(&v(x), &Term::Int(ty.max()));
        let view_bounds = definitions.declare_fn(
            &signature(over.clone(), 1, vec![], |p| {
                prelude.and_prop(lower(&p[0]), upper(&p[0]))
            }),
            |p| {
                let x = &p[0];
                Term::proof(Proof::Construct {
                    prop: prelude.and,
                    variant: 0,
                    params: vec![lower(x), upper(x)],
                    payload: vec![
                        Term::proof(Proof::Axiom(Axiom::ViewLower(ty, x.clone()))),
                        Term::proof(Proof::Axiom(Axiom::ViewUpper(ty, x.clone()))),
                    ],
                })
            },
        )?;

        // The bridge between a comparison and the order of the views, in
        // each direction, at each of the three comparisons. From the
        // comparison, cmp_reflect is the whole proof; towards it, the
        // boolean is decided by case analysis and cmp_reflect refutes the
        // wrong case.
        let reflect_true = move |op: CmpOp, p: &[Term]| {
            Proof::implies_elim(
                Proof::Axiom(Axiom::CmpReflect(cmp(op, &p[0], &p[1]), true)),
                given(&p[2]),
            )
        };
        let le_of_cmp = definitions.declare_fn(
            &signature(
                over.clone(),
                2,
                vec![Box::new(move |p| is(cmp(CmpOp::Le, &p[0], &p[1]), true))],
                |p| view_le(p, 0, 1),
            ),
            |p| Term::proof(reflect_true(CmpOp::Le, p)),
        )?;
        let cmp_of_le = definitions.declare_fn(
            &signature(
                over.clone(),
                2,
                vec![Box::new(move |p| view_le(p, 0, 1))],
                |p| is(cmp(CmpOp::Le, &p[0], &p[1]), true),
            ),
            |p| Term::proof(cmp_true_of(&cmp(CmpOp::Le, &p[0], &p[1]), given(&p[2]))),
        )?;
        let lt_of_cmp = definitions.declare_fn(
            &signature(
                over.clone(),
                2,
                vec![Box::new(move |p| is(cmp(CmpOp::Lt, &p[0], &p[1]), true))],
                |p| view_lt(p, 0, 1),
            ),
            |p| Term::proof(reflect_true(CmpOp::Lt, p)),
        )?;
        let cmp_of_lt = definitions.declare_fn(
            &signature(
                over.clone(),
                2,
                vec![Box::new(move |p| view_lt(p, 0, 1))],
                |p| is(cmp(CmpOp::Lt, &p[0], &p[1]), true),
            ),
            |p| Term::proof(cmp_true_of(&cmp(CmpOp::Lt, &p[0], &p[1]), given(&p[2]))),
        )?;
        let eq_of_cmp = definitions.declare_fn(
            &signature(
                over.clone(),
                2,
                vec![Box::new(move |p| is(cmp(CmpOp::Eq, &p[0], &p[1]), true))],
                |p| same(&p[0], &p[1]),
            ),
            |p| {
                Term::proof(lemma(
                    view_injective,
                    vec![
                        p[0].clone(),
                        p[1].clone(),
                        Term::proof(reflect_true(CmpOp::Eq, p)),
                    ],
                ))
            },
        )?;
        let cmp_of_eq = definitions.declare_fn(
            &signature(
                over.clone(),
                2,
                vec![Box::new(move |p| same(&p[0], &p[1]))],
                |p| is(cmp(CmpOp::Eq, &p[0], &p[1]), true),
            ),
            |p| {
                // v(a) == v(b), by congruence from a == b.
                let va = v(&p[0]);
                let views_equal = Proof::transport(
                    given(&p[2]),
                    |hole| Term::eq(Type::Int, va.clone(), v(&hole)),
                    Proof::Refl(va.clone()),
                );
                Term::proof(cmp_true_of(&cmp(CmpOp::Eq, &p[0], &p[1]), views_equal))
            },
        )?;

        // The false case of each ordering, as the fact about the views it
        // gives: `!(v(a) <= v(b))` is a linear constraint, and so is
        // `!(v(a) < v(b))`, so a certificate turns each around.
        let reflect_false = move |op: CmpOp, p: &[Term]| {
            Proof::implies_elim(
                Proof::Axiom(Axiom::CmpReflect(cmp(op, &p[0], &p[1]), false)),
                given(&p[2]),
            )
        };
        let lt_of_not_le = definitions.declare_fn(
            &signature(
                over.clone(),
                2,
                vec![Box::new(move |p| is(cmp(CmpOp::Le, &p[0], &p[1]), false))],
                |p| view_lt(p, 1, 0),
            ),
            |p| {
                Term::proof(Proof::linear(
                    view_lt(p, 1, 0),
                    1,
                    vec![(reflect_false(CmpOp::Le, p), 1)],
                ))
            },
        )?;
        let le_of_not_lt = definitions.declare_fn(
            &signature(
                over.clone(),
                2,
                vec![Box::new(move |p| is(cmp(CmpOp::Lt, &p[0], &p[1]), false))],
                |p| view_le(p, 1, 0),
            ),
            |p| {
                Term::proof(Proof::linear(
                    view_le(p, 1, 0),
                    1,
                    vec![(reflect_false(CmpOp::Lt, p), 1)],
                ))
            },
        )?;

        families.push(MachineLemmas {
            le_refl,
            le_trans,
            le_of_lt,
            lt_of_le_of_ne,
            lt_irrefl,
            le_antisymm,
            view_injective,
            view_bounds,
            le_of_cmp,
            cmp_of_le,
            lt_of_cmp,
            cmp_of_lt,
            eq_of_cmp,
            cmp_of_eq,
            lt_of_not_le,
            le_of_not_lt,
        });
    }
    let machine: [MachineLemmas; 8] = families
        .try_into()
        .unwrap_or_else(|_| unreachable!("one family per machine type"));

    Ok((
        [
            int_le_of_lt,
            int_lt_of_le_of_ne,
            int_le_add_left,
            int_le_add_right,
            int_le_sub,
            int_mul_le_mul_nonneg,
        ],
        machine,
    ))
}

/// Case analysis on a natural through `nat_zero_or_succ`: `when_zero`
/// receives `k == 0`, and `when_succ` receives `j` and `k == succ(j)`.
fn by_zero_or_succ(
    zero_or_succ: FnId,
    k: &Term,
    goal: &Term,
    when_zero: impl FnOnce(Proof) -> Proof,
    when_succ: impl FnOnce(Term, Proof) -> Proof,
) -> Proof {
    Proof::CaseProof {
        scrutinee: Box::new(lemma(zero_or_succ, vec![k.clone()])),
        goal: goal.clone(),
        arms: vec![
            Proof::arm(1, 0, |payload, _| {
                when_zero(Proof::OfTerm(payload[0].clone()))
            }),
            Proof::arm(1, 0, |payload, _| Proof::ExistsElim {
                exists: Box::new(Proof::OfTerm(payload[0].clone())),
                goal: goal.clone(),
                arm: Proof::arm(1, 1, |js, facts| when_succ(js[0].clone(), facts[0].clone())),
            }),
        ],
    }
}
