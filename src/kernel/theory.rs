//! The beginning of the kernel-level prelude: lemmas about `Int` and about
//! every machine integer type over its model in `Int`, proved from the
//! axioms and checked by the kernel like any other declaration. Nothing
//! here is trusted. If a proof below were wrong, `declare` would fail.
//!
//! These are written in the kernel's own term language so that they exist
//! before any source is elaborated. The lemmas about a machine type `T` are
//! stated over the views, `int_le(view[T](a), view[T](b))`, and are one
//! family declared once at each type, with three more at the unsigned types;
//! facts about the ordering of machine values come from the axioms of `Int`
//! and of the model, never by enumerating values. `Theory::lemma_names`
//! lists them under the names the source language calls them by.

use super::defs::{Definitions, Prelude};
use super::derive::{Chain, symm_at};
use super::error::KernelError;
use super::machine::MachineInt;
use super::ops::Op;
use super::term::{Axiom, CmpOp, FnId, Proof, Term, Type};

/// Identities of the declared lemmas.
#[derive(Clone, Copy, Debug)]
pub struct Theory {
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
/// `UnsignedLemmas`, the three more that hold at an unsigned type, with
/// `MachineLemmas::names`, which names each at a type as `u16_le_trans`.
/// The names are the source names: `<type>_<lemma>`.
macro_rules! machine_lemmas {
    (
        $($(#[$doc:meta])* $field:ident),* $(,)?;
        $($(#[$udoc:meta])* $ufield:ident),* $(,)?
    ) => {
        /// The lemmas about one machine type `T`, over the views: `v(x)`
        /// stands for `view[T](x)`, `a < b` for `int_lt(a, b)`, `!P` for
        /// `P => False`, `le[T]`, `lt[T]`, `eq[T]` for the runtime
        /// comparisons `Prim::Cmp`, and `succ(a)` for `wrapping_add[T](a, 1)`.
        /// Each premise is a proof parameter.
        #[derive(Clone, Copy, Debug)]
        pub struct MachineLemmas {
            $($(#[$doc])* pub $field: FnId,)*
            /// The lemmas that hold at an unsigned type only; `None` at a
            /// signed one.
            pub unsigned: Option<UnsignedLemmas>,
        }

        /// The lemmas about an unsigned machine type `T` that a signed type
        /// does not have: with `min(T) == 0`, a difference under its minuend
        /// stays in range, which fails at a signed type, as `127i8 - -128i8`
        /// shows. The notation is that of `MachineLemmas`.
        #[derive(Clone, Copy, Debug)]
        pub struct UnsignedLemmas {
            $($(#[$udoc])* pub $ufield: FnId,)*
        }

        impl MachineLemmas {
            /// The lemmas under their names at the given type.
            pub fn names(&self, ty: MachineInt) -> Vec<(&'static str, FnId)> {
                macro_rules! at {
                    ($prefix:literal) => {{
                        let mut names = vec![$((concat!($prefix, stringify!($field)), self.$field),)*];
                        if let Some(unsigned) = &self.unsigned {
                            names.extend([$((concat!($prefix, stringify!($ufield)), unsigned.$ufield),)*]);
                        }
                        names
                    }};
                }
                match ty {
                    MachineInt::U8 => at!("u8_"),
                    MachineInt::U16 => at!("u16_"),
                    MachineInt::U32 => at!("u32_"),
                    MachineInt::U64 => at!("u64_"),
                    MachineInt::I8 => at!("i8_"),
                    MachineInt::I16 => at!("i16_"),
                    MachineInt::I32 => at!("i32_"),
                    MachineInt::I64 => at!("i64_"),
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
    /// `v(a) < v(b) => v(succ(a)) <= v(b)`: below another value, the
    /// successor does not wrap
    succ_le_of_lt,
    /// `a ==[T] b => b ==[T] a`
    eq_symm;
    /// `v(0) <= v(a)`, stated about the literal `0` of `T` so that it is
    /// the claim `0 <= a` reads as
    zero_le,
    /// `v(b) <= v(a) => v(wrapping_sub[T](a, b)) <= v(a)`
    sub_le,
    /// `v(b) <= v(c) => v(c) <= v(a) => v(wrapping_sub[T](a, c)) <= v(wrapping_sub[T](a, b))`
    sub_le_sub,
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
    /// The elaborator exposes exactly this table to source. The names are
    /// distinct, and a test lists them so that a rename is deliberate.
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

        // v(a) < v(b) gives v(succ(a)) <= v(b): the successor is
        // wrapping_add[T](a, 1), whose model is wrap of v(a) + v(1); below
        // v(b) that sum is in range, so its view is the sum itself, by
        // view_wrap, and the literal step gives v(1) == 1.
        let succ_le_of_lt = definitions.declare_fn(
            &signature(
                over.clone(),
                2,
                vec![Box::new(move |p| view_lt(p, 0, 1))],
                |p| le(&v(&Term::successor(ty, p[0].clone())), &v(&p[1])),
            ),
            |p| {
                let (a, b) = (&p[0], &p[1]);
                let one = Term::machine_int(ty, 1);
                let operands = [a.clone(), one.clone()];
                let exact = add(&v(a), &v(&one));
                let one_is = || Proof::Literal(v(&one));
                let in_range = (
                    Proof::linear(
                        le(&Term::Int(ty.min()), &exact),
                        1,
                        vec![
                            (Proof::Axiom(Axiom::ViewLower(ty, a.clone())), 1),
                            (one_is(), -1),
                        ],
                    ),
                    Proof::linear(
                        le(&exact, &Term::Int(ty.max())),
                        1,
                        vec![
                            (given(&p[2]), 1),
                            (Proof::Axiom(Axiom::ViewUpper(ty, b.clone())), 1),
                            (one_is(), 1),
                        ],
                    ),
                );
                let exact_view = view_of_row(ty, Op::WrappingAdd, &operands, in_range);
                Term::proof(Proof::linear(
                    le(&v(&Term::successor(ty, a.clone())), &v(b)),
                    1,
                    vec![(exact_view, 1), (given(&p[2]), 1), (one_is(), 1)],
                ))
            },
        )?;

        let eq_symm = definitions.declare_fn(
            &signature(
                over.clone(),
                2,
                vec![Box::new(move |p| same(&p[0], &p[1]))],
                |p| same(&p[1], &p[0]),
            ),
            |p| Term::proof(symm_at(&over, &p[0], given(&p[2]))),
        )?;

        // At an unsigned type, min(T) is 0 and every view is non-negative,
        // so a difference under its minuend stays in range and its view is
        // the exact difference; each bound is a linear certificate from the
        // premises and the range axioms of the operands.
        let unsigned = if ty.signed() {
            None
        } else {
            let sub = |a: &Term, b: &Term| Term::int_sub(a.clone(), b.clone());
            let difference =
                move |a: &Term, b: &Term| Term::op(Op::WrappingSub, ty, vec![a.clone(), b.clone()]);
            // view_lower says 0 <= v(a) with the number 0; the literal
            // step carries it to the view of the literal 0.
            let zero_literal = Term::machine_int(ty, 0);
            let zero_le = definitions.declare_fn(
                &signature(over.clone(), 1, vec![], |p| {
                    le(&v(&zero_literal), &v(&p[0]))
                }),
                |p| {
                    let view_a = v(&p[0]);
                    let zero_is = symm_at(
                        &Type::Int,
                        &v(&zero_literal),
                        Proof::Literal(v(&zero_literal)),
                    );
                    Term::proof(Proof::transport(
                        zero_is,
                        |hole| le(&hole, &view_a),
                        Proof::Axiom(Axiom::ViewLower(ty, p[0].clone())),
                    ))
                },
            )?;
            // The view of a - b, for b <= a, from proofs of the two bounds.
            let exact_difference = move |a: &Term, b: &Term, below: Proof| {
                let exact = sub(&v(a), &v(b));
                let in_range = (
                    Proof::linear(le(&Term::Int(ty.min()), &exact), 1, vec![(below, 1)]),
                    Proof::linear(
                        le(&exact, &Term::Int(ty.max())),
                        1,
                        vec![
                            (Proof::Axiom(Axiom::ViewUpper(ty, a.clone())), 1),
                            (Proof::Axiom(Axiom::ViewLower(ty, b.clone())), 1),
                        ],
                    ),
                );
                view_of_row(ty, Op::WrappingSub, &[a.clone(), b.clone()], in_range)
            };
            let sub_le = definitions.declare_fn(
                &signature(
                    over.clone(),
                    2,
                    vec![Box::new(move |p| view_le(p, 1, 0))],
                    |p| le(&v(&difference(&p[0], &p[1])), &v(&p[0])),
                ),
                |p| {
                    let (a, b) = (&p[0], &p[1]);
                    Term::proof(Proof::linear(
                        le(&v(&difference(a, b)), &v(a)),
                        1,
                        vec![
                            (exact_difference(a, b, given(&p[2])), 1),
                            (Proof::Axiom(Axiom::ViewLower(ty, b.clone())), 1),
                        ],
                    ))
                },
            )?;
            let sub_le_sub = definitions.declare_fn(
                &signature(
                    over.clone(),
                    3,
                    vec![
                        Box::new(move |p| view_le(p, 1, 2)),
                        Box::new(move |p| view_le(p, 2, 0)),
                    ],
                    |p| le(&v(&difference(&p[0], &p[2])), &v(&difference(&p[0], &p[1]))),
                ),
                |p| {
                    let (a, b, c) = (&p[0], &p[1], &p[2]);
                    let (b_le_c, c_le_a) = (given(&p[3]), given(&p[4]));
                    // b <= a, from b <= c <= a.
                    let b_le_a = Proof::linear(
                        le(&v(b), &v(a)),
                        1,
                        vec![(b_le_c.clone(), 1), (c_le_a.clone(), 1)],
                    );
                    Term::proof(Proof::linear(
                        le(&v(&difference(a, c)), &v(&difference(a, b))),
                        1,
                        vec![
                            (exact_difference(a, c, c_le_a), 1),
                            (exact_difference(a, b, b_le_a), -1),
                            (b_le_c, 1),
                        ],
                    ))
                },
            )?;
            Some(UnsignedLemmas {
                zero_le,
                sub_le,
                sub_le_sub,
            })
        };

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
            succ_le_of_lt,
            eq_symm,
            unsigned,
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

/// `view[T](op[T](operands)) ==[Int] e`, for a row of the table and the
/// exact result `e` of its operands' views, from proofs that `e` is in the
/// range of `T`: the row is `wrap[T](e)` by `op_model`, and the view of
/// that is `e` by `view_wrap` under the two bounds.
fn view_of_row(ty: MachineInt, op: Op, operands: &[Term], in_range: (Proof, Proof)) -> Proof {
    let row = op.row(ty).expect("the row exists at this type");
    let exact = row.exact_term(operands);
    let applied = row.applied(operands);
    let view_of_wrapped = Proof::implies_elim(
        Proof::implies_elim(Proof::Axiom(Axiom::ViewWrap(ty, exact.clone())), in_range.0),
        in_range.1,
    );
    // wrap(e) == op(operands), to move the view from the one to the other.
    let back = symm_at(
        &Type::machine(ty),
        &applied,
        Proof::Axiom(Axiom::OpModel(op, ty, operands.to_vec())),
    );
    Proof::transport(
        back,
        |hole| Term::eq(Type::Int, Term::view(ty, hole), exact.clone()),
        view_of_wrapped,
    )
}
