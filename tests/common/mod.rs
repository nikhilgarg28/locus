//! Typed trees shared by the integration tests: the specification's
//! programs in source shape.
#![allow(dead_code)]

use locus::kernel::derive::symm_at;
use locus::kernel::theory::{self, Theory};
use locus::kernel::{
    Axiom, CmpOp, Definitions, EnumId, FnId, HypId, MachineInt, Op, Prelude, Prim, Proof, Term,
    Type, VarId,
};
use locus::typed::{
    Binder, Block, CompareOp, EnumItem, Expr, FnItem, FnRef, Join, Joined, MatchArm, Pattern,
    Place, Session, Step, Stmt, VariantItem,
};

pub fn setup() -> (Session, Prelude, Theory) {
    let (mut definitions, prelude) = Definitions::with_prelude();
    let theory = theory::declare(&mut definitions, &prelude).expect("the theory checks");
    (Session::new(definitions), prelude, theory)
}

pub fn u8_eq(left: Term, right: Term) -> Term {
    Term::eq(Type::U8, left, right)
}

pub fn add_one(term: Term) -> Term {
    Term::successor(MachineInt::U8, term)
}

/// `view[u8](x)`: the value of a byte in the logic.
pub fn view(term: Term) -> Term {
    Term::view(MachineInt::U8, term)
}

/// `a <= b` between bytes, over their views.
pub fn u8_le(left: Term, right: Term) -> Term {
    Term::int_le(view(left), view(right))
}

/// `a < b` between bytes, over their views.
pub fn u8_lt(left: Term, right: Term) -> Term {
    Term::int_lt(view(left), view(right))
}

/// `view[u8](a) == view[u8](b)`: what `cmp_reflect` says of `eq[u8](a, b)`.
pub fn views_eq(left: Term, right: Term) -> Term {
    Term::eq(Type::Int, view(left), view(right))
}

/// `eq[u8](a, b)`, the runtime test.
pub fn u8_test_eq(left: Term, right: Term) -> Term {
    Term::cmp(CmpOp::Eq, MachineInt::U8, left, right)
}

/// A comparison of bytes in the typed tree.
pub fn compare_u8(op: CompareOp, left: Expr, right: Expr) -> Expr {
    Expr::Compare {
        op,
        ty: Type::U8,
        left: Box::new(left),
        right: Box::new(right),
    }
}

pub fn lemma(id: FnId, arguments: Vec<Term>) -> Proof {
    Proof::OfTerm(Term::call(Term::Fn(id), arguments))
}

/// `value.wrapping_add(1)`
pub fn plus_one(value: Expr) -> Expr {
    Expr::Method {
        prim: Prim::Op(Op::WrappingAdd, MachineInt::U8),
        receiver: Box::new(value),
        arguments: vec![Expr::u8(1)],
    }
}

pub fn block(stmts: Vec<Stmt>, tail: Expr) -> Block {
    Block {
        stmts,
        tail: Some(Box::new(tail)),
    }
}

pub fn let_(binder: &Binder, equation: HypId, value: Expr) -> Stmt {
    Stmt::Let {
        pattern: Pattern::Bind {
            binder: binder.clone(),
            equation,
            mutable: false,
        },
        value,
    }
}

/// `target.index`, a byte.
pub fn field(target: Expr, index: usize) -> Expr {
    Expr::Field {
        target: Box::new(target),
        index,
        name: None,
        ty: Type::U8,
    }
}

/// `(out: u8, @[claim(out)])`
pub fn data_with_evidence(claim: impl Fn(Term) -> Term + 'static) -> Type {
    Type::tuple(move |earlier| match earlier {
        [] => Some(Type::U8),
        [out] => Some(Type::proof(claim(out.clone()))),
        _ => None,
    })
}

pub fn exec_id(reference: FnRef) -> locus::exec::ExecFnId {
    match reference {
        FnRef::Exec(id) => id,
        FnRef::Math(_) => panic!("expected an ordinary function"),
    }
}

/// fn increment(n: u8) -> (out: u8, @[out == n.wrapping_add(1)]) {
///     let out = n.wrapping_add(1);
///     (out, _)
/// }
pub fn increment(math: bool) -> FnItem {
    let n = Binder::new("n", Type::U8);
    let out = Binder::new("out", Type::U8);
    let out_is = HypId::fresh();
    let n_term = n.term();
    let result = data_with_evidence(move |out| u8_eq(out, add_one(n_term.clone())));
    FnItem {
        name: "increment".into(),
        math,
        params: vec![n.clone()],
        result: result.clone(),
        body: block(
            vec![let_(&out, out_is, plus_one(Expr::var(&n)))],
            Expr::Tuple {
                ty: result,
                fields: vec![Expr::var(&out), Expr::Proof(Proof::hyp(out_is))],
            },
        ),
    }
}

/// fn preserve(n: u8) -> (out: u8, @[out == n]) {
///     if n == 0 { (0, _) } else { (n, _) }
/// }
pub fn preserve(theory: Theory, math: bool, use_the_fact: bool) -> FnItem {
    let n = Binder::new("n", Type::U8);
    let n_term = n.term();
    let result = data_with_evidence({
        let n_term = n_term.clone();
        move |out| u8_eq(out, n_term.clone())
    });
    let (then_fact, else_fact) = (HypId::fresh(), HypId::fresh());
    // The fact of the then branch is about the comparison the condition
    // performs; reflection turns it into an equality of the views, and the
    // injectivity of the view into n == 0.
    let comparison = u8_test_eq(n_term.clone(), Term::U8(0));
    let views_equal = Proof::implies_elim(
        Proof::Axiom(Axiom::CmpReflect(comparison, true)),
        Proof::hyp(then_fact),
    );
    let n_is_zero = lemma(
        theory.machine(MachineInt::U8).view_injective,
        vec![n_term.clone(), Term::U8(0), Term::proof(views_equal)],
    );
    let target = n_term.clone();
    let zero_is_n = Proof::transport(
        n_is_zero,
        |hole| u8_eq(hole, target.clone()),
        Proof::Refl(n_term.clone()),
    );
    let evidence = if use_the_fact {
        zero_is_n
    } else {
        Proof::Refl(Term::U8(0))
    };
    let pair = |value: Expr, proof: Proof| Expr::Tuple {
        ty: result.clone(),
        fields: vec![value, Expr::Proof(proof)],
    };
    FnItem {
        name: "preserve".into(),
        math,
        params: vec![n.clone()],
        result: result.clone(),
        body: Block {
            stmts: vec![],
            tail: Some(Box::new(Expr::If {
                condition: Box::new(Expr::Compare {
                    op: CompareOp::Eq,
                    ty: Type::U8,
                    left: Box::new(Expr::var(&n)),
                    right: Box::new(Expr::u8(0)),
                }),
                then_fact,
                else_fact,
                then_block: block(vec![], pair(Expr::u8(0), evidence)),
                else_block: block(vec![], pair(Expr::var(&n), Proof::Refl(n_term))),
                ty: result.clone(),
                result: VarId::fresh(),
                joined: None,
            })),
        },
    }
}

/// fn bounded_walk(limit: u8) -> (value: u8, evidence: @[value <= limit]) of
/// specification section 10.4, in source shape.
pub fn bounded_walk(prelude: Prelude, theory: Theory, carry_the_invariant: bool) -> FnItem {
    let limit = Binder::new("limit", Type::U8);
    let limit_term = limit.term();
    let result = data_with_evidence({
        let limit_term = limit_term.clone();
        move |value| u8_le(value, limit_term.clone())
    });
    let i = Binder::new("i", Type::U8);
    let bound = Binder::new("bound", Type::proof(u8_le(i.term(), limit_term.clone())));
    let next = Binder::new("next", Type::U8);
    let next_bound = Binder::new(
        "next_bound",
        Type::proof(u8_le(next.term(), limit_term.clone())),
    );
    let differs = Binder::new(
        "differs",
        Type::proof(prelude.not_prop(Term::eq(
            Type::Int,
            view(i.term()),
            view(limit_term.clone()),
        ))),
    );
    let below = Binder::new("below", Type::proof(u8_lt(i.term(), limit_term.clone())));
    let (then_fact, else_fact, next_is) = (HypId::fresh(), HypId::fresh(), HypId::fresh());
    let comparison = u8_test_eq(i.term(), limit_term.clone());
    let as_proof = |binder: &Binder| Proof::OfTerm(binder.term());
    let limit_in = limit_term.clone();
    let carried = if carry_the_invariant {
        Expr::var(&next_bound)
    } else {
        Expr::var(&bound)
    };
    let keep_walking = block(
        vec![
            let_(
                &differs,
                HypId::fresh(),
                Expr::Proof(Proof::implies_elim(
                    Proof::Axiom(Axiom::CmpReflect(comparison, false)),
                    Proof::hyp(else_fact),
                )),
            ),
            let_(
                &below,
                HypId::fresh(),
                Expr::Proof(lemma(
                    theory.machine(MachineInt::U8).lt_of_le_of_ne,
                    vec![
                        i.term(),
                        limit_term.clone(),
                        Term::proof(as_proof(&bound)),
                        Term::proof(as_proof(&differs)),
                    ],
                )),
            ),
            let_(&next, next_is, plus_one(Expr::var(&i))),
            let_(
                &next_bound,
                HypId::fresh(),
                Expr::Proof(Proof::transport(
                    symm_at(&Type::U8, &next.term(), Proof::hyp(next_is)),
                    |hole| u8_le(hole, limit_in.clone()),
                    lemma(
                        theory.machine(MachineInt::U8).succ_le_of_lt,
                        vec![i.term(), limit_term.clone(), Term::proof(as_proof(&below))],
                    ),
                )),
            ),
        ],
        Expr::Continue(vec![Expr::var(&next), carried]),
    );
    let stop = block(
        vec![],
        Expr::Break(Box::new(Expr::Tuple {
            ty: result.clone(),
            fields: vec![Expr::var(&i), Expr::var(&bound)],
        })),
    );
    FnItem {
        name: "bounded_walk".into(),
        math: false,
        params: vec![limit.clone()],
        result: result.clone(),
        body: Block {
            stmts: vec![],
            tail: Some(Box::new(Expr::Loop {
                state: vec![
                    (i.clone(), Expr::u8(0)),
                    (
                        bound.clone(),
                        Expr::Proof(lemma(
                            theory.machine(MachineInt::U8).unsigned.unwrap().zero_le,
                            vec![limit_term.clone()],
                        )),
                    ),
                ],
                result_ty: result,
                body: Block {
                    stmts: vec![],
                    tail: Some(Box::new(Expr::If {
                        condition: Box::new(Expr::Compare {
                            op: CompareOp::Eq,
                            ty: Type::U8,
                            left: Box::new(Expr::var(&i)),
                            right: Box::new(Expr::var(&limit)),
                        }),
                        then_fact,
                        else_fact,
                        then_block: stop,
                        else_block: keep_walking,
                        ty: Type::Tuple(vec![]),
                        result: VarId::fresh(),
                        joined: None,
                    })),
                },
                result: VarId::fresh(),
            })),
        },
    }
}

/// for i in 0..n (acc: u8 = 0, same: @[acc == i] = _) { continue(step(acc), _) }
/// where `step` is either a pure increment or a call to `increment`.
pub fn counting_loop(theory: Theory, math: bool, step: Option<locus::exec::ExecFnId>) -> FnItem {
    let n = Binder::new("n", Type::U8);
    let i = Binder::new("i", Type::U8);
    let acc = Binder::new("acc", Type::U8);
    let same = Binder::new("same", Type::proof(u8_eq(acc.term(), i.term())));
    let n_term = n.term();
    let result = data_with_evidence(move |total| u8_eq(total, n_term.clone()));

    let (stmts, stepped, stepped_term, because) = match step {
        None => {
            let value = add_one(acc.term());
            (
                vec![],
                plus_one(Expr::var(&acc)),
                value.clone(),
                Proof::Refl(value),
            )
        }
        Some(increment_id) => {
            // let r = increment(acc);   r.1 : r.0 == acc + 1
            let acc_term = acc.term();
            let r_type = data_with_evidence(move |out| u8_eq(out, add_one(acc_term.clone())));
            let r = Binder::new("r", r_type.clone());
            let stmts = vec![let_(
                &r,
                HypId::fresh(),
                Expr::CallFn {
                    id: increment_id,
                    name: "increment".into(),
                    arguments: vec![Expr::var(&acc)],
                    result: VarId::fresh(),
                    ty: r_type,
                },
            )];
            (
                stmts,
                field(Expr::var(&r), 0),
                Term::proj(r.term(), 0),
                Proof::OfTerm(Term::proj(r.term(), 1)),
            )
        }
    };
    // stepped == acc + 1 and acc == i give stepped == i + 1.
    let left = stepped_term.clone();
    let advanced = Proof::transport(
        Proof::OfTerm(same.term()),
        |hole| u8_eq(left.clone(), add_one(hole)),
        because,
    );
    FnItem {
        name: "count".into(),
        math,
        params: vec![n.clone()],
        result,
        body: Block {
            stmts: vec![],
            tail: Some(Box::new(Expr::For {
                index: i.clone(),
                lower: HypId::fresh(),
                upper: HypId::fresh(),
                lo: Box::new(Expr::u8(0)),
                hi: Box::new(Expr::var(&n)),
                ordered: lemma(
                    theory.machine(MachineInt::U8).unsigned.unwrap().zero_le,
                    vec![n.term()],
                ),
                state: vec![
                    (acc.clone(), Expr::u8(0)),
                    (same.clone(), Expr::Proof(Proof::Refl(Term::U8(0)))),
                ],
                body: block(stmts, Expr::Continue(vec![stepped, Expr::Proof(advanced)])),
                result: VarId::fresh(),
            })),
        },
    }
}

/// fn spin() -> @[false] { loop () -> @[false] { continue(); } }
pub fn spin(prelude: Prelude) -> FnItem {
    let falsehood = Type::proof(prelude.falsehood_prop());
    FnItem {
        name: "spin".into(),
        math: false,
        params: vec![],
        result: falsehood.clone(),
        body: Block {
            stmts: vec![],
            tail: Some(Box::new(Expr::Loop {
                state: vec![],
                result_ty: falsehood,
                body: block(vec![], Expr::Continue(vec![])),
                result: VarId::fresh(),
            })),
        },
    }
}

/// fn caller() -> u8 { let impossible = spin(); 0 }
pub fn caller_of_spin(prelude: Prelude, spin: locus::exec::ExecFnId) -> FnItem {
    let falsehood = Type::proof(prelude.falsehood_prop());
    let impossible = Binder::new("impossible", falsehood.clone());
    FnItem {
        name: "caller".into(),
        math: false,
        params: vec![],
        result: Type::U8,
        body: block(
            vec![let_(
                &impossible,
                HypId::fresh(),
                Expr::CallFn {
                    id: spin,
                    name: "spin".into(),
                    arguments: vec![],
                    result: VarId::fresh(),
                    ty: falsehood,
                },
            )],
            Expr::u8(0),
        ),
    }
}

/// enum Classified { Zero(value: u8, @[value == 0]), NonZero(value: u8, @[value != 0]) }
/// with the claims stated over the views, as reflecting the test gives them.
pub fn classified_enum(prelude: Prelude) -> EnumItem {
    let payload = |claim: fn(&Prelude, Term) -> Term| {
        let value = Binder::new("value", Type::U8);
        let evidence = Binder::new("evidence", Type::proof(claim(&prelude, value.term())));
        vec![value, evidence]
    };
    EnumItem {
        name: "Classified".into(),
        variants: vec![
            VariantItem {
                name: "Zero".into(),
                payload: payload(|_, value| views_eq(value, Term::U8(0))),
            },
            VariantItem {
                name: "NonZero".into(),
                payload: payload(|prelude, value| prelude.not_prop(views_eq(value, Term::U8(0)))),
            },
        ],
    }
}

/// fn classify(n: u8) -> Classified {
///     if n != 0 { Classified::NonZero(n, _) } else { Classified::Zero(n, _) }
/// }
/// The condition is a negation, so the then branch is the one where the
/// comparison n == 0 came out false.
pub fn classify(classified: EnumId) -> FnItem {
    let n = Binder::new("n", Type::U8);
    let comparison = u8_test_eq(n.term(), Term::U8(0));
    let (then_fact, else_fact) = (HypId::fresh(), HypId::fresh());
    let reflect = |flag: bool, fact: HypId| {
        Proof::implies_elim(
            Proof::Axiom(Axiom::CmpReflect(comparison.clone(), flag)),
            Proof::hyp(fact),
        )
    };
    let variant = |index: usize, name: &str, evidence: Proof| Expr::Variant {
        id: classified,
        enum_name: "Classified".into(),
        index,
        variant_name: name.into(),
        payload: vec![Expr::var(&n), Expr::Proof(evidence)],
    };
    FnItem {
        name: "classify".into(),
        math: false,
        params: vec![n.clone()],
        result: Type::Enum(classified),
        body: Block {
            stmts: vec![],
            tail: Some(Box::new(Expr::If {
                condition: Box::new(Expr::Compare {
                    op: CompareOp::Ne,
                    ty: Type::U8,
                    left: Box::new(Expr::var(&n)),
                    right: Box::new(Expr::u8(0)),
                }),
                then_fact,
                else_fact,
                then_block: block(vec![], variant(1, "NonZero", reflect(false, then_fact))),
                else_block: block(vec![], variant(0, "Zero", reflect(true, else_fact))),
                ty: Type::Enum(classified),
                result: VarId::fresh(),
                joined: None,
            })),
        },
    }
}

/// fn zero_or_self(m: u8) -> u8 {
///     match classify(m) { Classified::Zero(v, h) => v, Classified::NonZero(v, h) => v }
/// }
pub fn zero_or_self(
    prelude: Prelude,
    classified: EnumId,
    classify_id: locus::exec::ExecFnId,
) -> FnItem {
    let m = Binder::new("m", Type::U8);
    let arm = |name: &str, claim: fn(&Prelude, Term) -> Term| {
        let v = Binder::new("v", Type::U8);
        let h = Binder::new("h", Type::proof(claim(&prelude, v.term())));
        MatchArm {
            variant_name: name.into(),
            payload: vec![v.clone(), h],
            fact: HypId::fresh(),
            body: block(vec![], Expr::var(&v)),
        }
    };
    FnItem {
        name: "zero_or_self".into(),
        math: false,
        params: vec![m.clone()],
        result: Type::U8,
        body: Block {
            stmts: vec![],
            tail: Some(Box::new(Expr::Match {
                scrutinee: Box::new(Expr::CallFn {
                    id: classify_id,
                    name: "classify".into(),
                    arguments: vec![Expr::var(&m)],
                    result: VarId::fresh(),
                    ty: Type::Enum(classified),
                }),
                enum_name: "Classified".into(),
                arms: vec![
                    arm("Zero", |_, v| views_eq(v, Term::U8(0))),
                    arm("NonZero", |prelude, v| {
                        prelude.not_prop(views_eq(v, Term::U8(0)))
                    }),
                ],
                ty: Type::U8,
                result: VarId::fresh(),
                joined: None,
            })),
        },
    }
}

// --- Mutation: `let mut`, assignment, and joins (M2) ----------------------------------

pub fn let_mut(binder: &Binder, value: Expr) -> Stmt {
    Stmt::Let {
        pattern: Pattern::Bind {
            binder: binder.clone(),
            equation: HypId::fresh(),
            mutable: true,
        },
        value,
    }
}

/// `binding.path = value;`, giving the binding the version `version`.
pub fn assign(binding: &Binder, path: Vec<Step>, value: Expr, version: &Binder) -> Stmt {
    Stmt::Assign {
        place: Place {
            binding: binding.id,
            name: binding.name.clone(),
            path,
        },
        value,
        version: version.clone(),
        equation: HypId::fresh(),
    }
}

/// A step into a tuple of bytes with `arity` fields.
pub fn byte_tuple_step(index: usize, arity: usize) -> Step {
    Step {
        index,
        name: None,
        ty: Type::Tuple(vec![Type::U8; arity]),
        proof_fields: vec![false; arity],
    }
}

/// The join of a branch: each assigned binding with the version it has
/// afterwards, in declaration order.
pub fn joined(joins: Vec<(&Binder, &Binder)>) -> Joined {
    Joined {
        tuple: VarId::fresh(),
        joins: joins
            .into_iter()
            .map(|(binding, version)| Join {
                binding: binding.id,
                version: version.clone(),
                equation: HypId::fresh(),
            })
            .collect(),
        equation: HypId::fresh(),
    }
}

/// `if condition { then } else { otherwise }` of type `ty`.
pub fn if_(
    condition: Expr,
    then_block: Block,
    else_block: Block,
    ty: Type,
    joined: Option<Joined>,
) -> Expr {
    Expr::If {
        condition: Box::new(condition),
        then_fact: HypId::fresh(),
        else_fact: HypId::fresh(),
        then_block,
        else_block,
        ty,
        result: VarId::fresh(),
        joined,
    }
}

pub fn unit_block(stmts: Vec<Stmt>) -> Block {
    Block { stmts, tail: None }
}

/// `n == 0`
pub fn is_zero(n: &Binder) -> Expr {
    compare_u8(CompareOp::Eq, Expr::var(n), Expr::u8(0))
}

/// A byte pair.
pub fn pair_type() -> Type {
    Type::Tuple(vec![Type::U8, Type::U8])
}

pub fn pair(left: Expr, right: Expr) -> Expr {
    Expr::Tuple {
        ty: pair_type(),
        fields: vec![left, right],
    }
}

/// fn straight(n: u8) -> u8 { let mut x = n; x = x + 1; x = x + 1; x }
pub fn straight_line_mutation() -> FnItem {
    let n = Binder::new("n", Type::U8);
    let x = Binder::new("x", Type::U8);
    let x1 = Binder::new("x", Type::U8);
    let x2 = Binder::new("x", Type::U8);
    FnItem {
        name: "straight".into(),
        math: false,
        params: vec![n.clone()],
        result: Type::U8,
        body: block(
            vec![
                let_mut(&x, Expr::var(&n)),
                assign(&x, vec![], plus_one(Expr::var(&x)), &x1),
                assign(&x, vec![], plus_one(Expr::var(&x1)), &x2),
            ],
            Expr::var(&x2),
        ),
    }
}

/// fn branching(n: u8) -> (u8, u8) {
///     let mut a = n; let mut b = 0;
///     if n == 0 { b = 1; a = 2; } else { a = a + 3; }
///     (a, b)
/// }
/// The join carries `a` and `b`, in declaration order, and a unit value.
pub fn branching_mutation() -> FnItem {
    let n = Binder::new("n", Type::U8);
    let a = Binder::new("a", Type::U8);
    let b = Binder::new("b", Type::U8);
    let (a_then, b_then, a_else) = (
        Binder::new("a", Type::U8),
        Binder::new("b", Type::U8),
        Binder::new("a", Type::U8),
    );
    let (a_join, b_join) = (Binder::new("a", Type::U8), Binder::new("b", Type::U8));
    let branch = if_(
        is_zero(&n),
        unit_block(vec![
            assign(&b, vec![], Expr::u8(1), &b_then),
            assign(&a, vec![], Expr::u8(2), &a_then),
        ]),
        unit_block(vec![assign(
            &a,
            vec![],
            Expr::Method {
                prim: Prim::Op(Op::WrappingAdd, MachineInt::U8),
                receiver: Box::new(Expr::var(&a)),
                arguments: vec![Expr::u8(3)],
            },
            &a_else,
        )]),
        Type::Tuple(vec![]),
        Some(joined(vec![(&a, &a_join), (&b, &b_join)])),
    );
    FnItem {
        name: "branching".into(),
        math: false,
        params: vec![n.clone()],
        result: pair_type(),
        body: block(
            vec![
                let_mut(&a, Expr::var(&n)),
                let_mut(&b, Expr::u8(0)),
                Stmt::Expr(branch),
            ],
            pair(Expr::var(&a_join), Expr::var(&b_join)),
        ),
    }
}

/// fn touch(n: u8) -> (u8, u8) { let mut p = (n, n); p.0 = { p.1 = 7; 3 }; p }
/// Case 9: the right side changes what the left side names.
pub fn right_side_changes_the_place() -> FnItem {
    let n = Binder::new("n", Type::U8);
    let p = Binder::new("p", pair_type());
    let p_inner = Binder::new("p", pair_type());
    let p_outer = Binder::new("p", pair_type());
    let right = Expr::Block(block(
        vec![assign(
            &p,
            vec![byte_tuple_step(1, 2)],
            Expr::u8(7),
            &p_inner,
        )],
        Expr::u8(3),
    ));
    FnItem {
        name: "touch".into(),
        math: false,
        params: vec![n.clone()],
        result: pair_type(),
        body: block(
            vec![
                let_mut(&p, pair(Expr::var(&n), Expr::var(&n))),
                assign(&p, vec![byte_tuple_step(0, 2)], right, &p_outer),
            ],
            Expr::var(&p_outer),
        ),
    }
}
