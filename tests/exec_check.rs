//! The exec checker on hand-built check IR (docs/ir-architecture.md): the
//! programs of the specification, written out the way `lower` will produce
//! them, and the ways they must be rejected.

use std::rc::Rc;

use locus::exec::{Arm, Block, ExecError, ExecFn, ExecFnId, ForStmt, Program, Stmt, Tail};
use locus::kernel::derive::symm_at;
use locus::kernel::theory::{self, Theory};
use locus::kernel::{
    Axiom, Definitions, EnumId, HypId, KernelError, Prelude, Prim, Proof, Term, Type, VarId,
};

struct World {
    definitions: Rc<Definitions>,
    prelude: Prelude,
    theory: Theory,
    classified: EnumId,
}

fn world() -> World {
    let (mut definitions, prelude) = Definitions::with_prelude();
    let theory = theory::declare(&mut definitions, &prelude).expect("the theory checks");
    // enum Classified { Zero(value: u8, @[value == 0]), NonZero(value: u8, @[value != 0]) }
    let payload = |claim: fn(&Prelude, Term) -> Term| {
        Type::tuple(move |earlier| match earlier {
            [] => Some(Type::U8),
            [value] => Some(Type::proof(claim(&prelude, value.clone()))),
            _ => None,
        })
    };
    let classified = definitions
        .declare_enum(&[
            payload(|_, value| u8_eq(value, Term::U8(0))),
            payload(|prelude, value| prelude.not_prop(u8_eq(value, Term::U8(0)))),
        ])
        .unwrap();
    World {
        definitions: Rc::new(definitions),
        prelude,
        theory,
        classified,
    }
}

fn u8_eq(left: Term, right: Term) -> Term {
    Term::eq(Type::U8, left, right)
}

fn add_one(term: Term) -> Term {
    Term::wrapping_add(term, Term::U8(1))
}

fn var() -> (VarId, Term) {
    let id = VarId::fresh();
    (id, Term::var(id))
}

fn lemma(id: locus::kernel::FnId, arguments: Vec<Term>) -> Proof {
    Proof::OfTerm(Term::call(Term::Fn(id), arguments))
}

fn block(stmts: Vec<Stmt>, tail: Tail) -> Block {
    Block { stmts, tail }
}

/// `(out: u8, @[out == n.wrapping_add(1)])`
fn increment_result(n: &Term) -> Type {
    let n = n.clone();
    Type::tuple(move |earlier| match earlier {
        [] => Some(Type::U8),
        [out] => Some(Type::proof(u8_eq(out.clone(), add_one(n.clone())))),
        _ => None,
    })
}

/// fn increment(n: u8) -> (out: u8, @[out == n.wrapping_add(1)]) {
///     let out = n.wrapping_add(1);
///     (out, _)
/// }
fn increment() -> ExecFn {
    let (n_id, n) = var();
    let (out_id, out) = var();
    let out_is = HypId::fresh();
    ExecFn {
        signature: Type::function(1, |params| match params {
            [] => Type::U8,
            [n] => increment_result(n),
            _ => unreachable!(),
        }),
        params: vec![n_id],
        body: block(
            vec![Stmt::Let {
                var: out_id,
                equation: out_is,
                value: add_one(n.clone()),
            }],
            Tail::Value(Term::tuple(
                &increment_result(&n),
                vec![out, Term::proof(Proof::hyp(out_is))],
            )),
        ),
    }
}

// --- Accepted programs -------------------------------------------------------

#[test]
fn a_dependent_result_and_a_caller_that_reuses_its_evidence() {
    let world = world();
    let mut program = Program::new(world.definitions);
    let increment = program.declare(increment()).unwrap();

    // fn twice(n: u8) -> (out: u8, @[out == n.wrapping_add(1).wrapping_add(1)]) {
    //     let first = increment(n);
    //     let second = increment(first.0);
    //     (second.0, _)
    // }
    let result = |n: &Term| {
        let n = n.clone();
        Type::tuple(move |earlier| match earlier {
            [] => Some(Type::U8),
            [out] => Some(Type::proof(u8_eq(out.clone(), add_one(add_one(n.clone()))))),
            _ => None,
        })
    };
    let twice = |use_evidence: bool| {
        let (n_id, n) = var();
        let (first_id, first) = var();
        let (second_id, second) = var();
        // second.0 == first.0 + 1, and first.0 == n + 1.
        let first_fact = Proof::OfTerm(Term::proj(first.clone(), 1));
        let second_fact = Proof::OfTerm(Term::proj(second.clone(), 1));
        let left = Term::proj(second.clone(), 0);
        let combined = Proof::transport(
            first_fact,
            |hole| u8_eq(left.clone(), add_one(hole)),
            second_fact,
        );
        let evidence = if use_evidence {
            combined
        } else {
            // A call's result is not equal to anything by computation.
            Proof::Refl(Term::proj(second.clone(), 0))
        };
        ExecFn {
            signature: Type::function(1, |params| match params {
                [] => Type::U8,
                [n] => result(n),
                _ => unreachable!(),
            }),
            params: vec![n_id],
            body: block(
                vec![
                    Stmt::Call {
                        var: first_id,
                        callee: increment,
                        arguments: vec![n.clone()],
                    },
                    Stmt::Call {
                        var: second_id,
                        callee: increment,
                        arguments: vec![Term::proj(first, 0)],
                    },
                ],
                Tail::Value(Term::tuple(
                    &result(&n),
                    vec![Term::proj(second, 0), Term::proof(evidence)],
                )),
            ),
        }
    };
    assert!(program.declare(twice(true)).is_ok());
    assert!(matches!(
        program.declare(twice(false)),
        Err(ExecError::Kernel(KernelError::ProofMismatch { .. }))
    ));
}

/// fn preserve(n: u8) -> (out: u8, @[out == n]) {
///     if n == 0 { (0, _) } else { (n, _) }
/// }
fn preserve(use_the_fact: bool) -> ExecFn {
    let (n_id, n) = var();
    let result = |n: &Term| {
        let n = n.clone();
        Type::tuple(move |earlier| match earlier {
            [] => Some(Type::U8),
            [out] => Some(Type::proof(u8_eq(out.clone(), n.clone()))),
            _ => None,
        })
    };
    let comparison = Term::prim(Prim::U8Eq, vec![n.clone(), Term::U8(0)]);
    let (when_false, when_true) = (HypId::fresh(), HypId::fresh());
    let n_is_zero = Proof::implies_elim(
        Proof::Axiom(Axiom::Reflect(comparison.clone(), true)),
        Proof::hyp(when_true),
    );
    let target = n.clone();
    let zero_is_n = Proof::transport(
        n_is_zero,
        |hole| u8_eq(hole, target.clone()),
        Proof::Refl(n.clone()),
    );
    let evidence = if use_the_fact {
        zero_is_n
    } else {
        Proof::Refl(Term::U8(0))
    };
    ExecFn {
        signature: Type::function(1, |params| match params {
            [] => Type::U8,
            [n] => result(n),
            _ => unreachable!(),
        }),
        params: vec![n_id],
        body: block(
            vec![],
            Tail::Match {
                scrutinee: comparison,
                arms: vec![
                    Arm {
                        payload: vec![],
                        fact: when_false,
                        body: block(
                            vec![],
                            Tail::Value(Term::tuple(
                                &result(&n),
                                vec![n.clone(), Term::proof(Proof::Refl(n.clone()))],
                            )),
                        ),
                    },
                    Arm {
                        payload: vec![],
                        fact: when_true,
                        body: block(
                            vec![],
                            Tail::Value(Term::tuple(
                                &result(&n),
                                vec![Term::U8(0), Term::proof(evidence)],
                            )),
                        ),
                    },
                ],
            },
        ),
    }
}

#[test]
fn each_branch_of_an_if_learns_the_condition() {
    let world = world();
    let mut program = Program::new(world.definitions);
    assert!(program.declare(preserve(true)).is_ok());
    assert!(matches!(
        program.declare(preserve(false)),
        Err(ExecError::Kernel(KernelError::ProofMismatch { .. }))
    ));
}

#[test]
fn an_enum_carries_the_decision_and_its_evidence() {
    // fn classify(n: u8) -> Classified {
    //     if n == 0 { Classified::Zero(n, _) } else { Classified::NonZero(n, _) }
    // }
    let world = world();
    let mut program = Program::new(Rc::clone(&world.definitions));
    let (n_id, n) = var();
    let comparison = Term::prim(Prim::U8Eq, vec![n.clone(), Term::U8(0)]);
    let (when_false, when_true) = (HypId::fresh(), HypId::fresh());
    let reflect = |flag: bool, fact: HypId| {
        Proof::implies_elim(
            Proof::Axiom(Axiom::Reflect(comparison.clone(), flag)),
            Proof::hyp(fact),
        )
    };
    let variant = |index: usize, evidence: Proof| {
        Term::Variant(
            world.classified,
            index,
            vec![n.clone(), Term::proof(evidence)],
        )
    };
    let classify = ExecFn {
        signature: Type::function(1, |params| match params {
            [] => Type::U8,
            _ => Type::Enum(world.classified),
        }),
        params: vec![n_id],
        body: block(
            vec![],
            Tail::Match {
                scrutinee: comparison.clone(),
                arms: vec![
                    Arm {
                        payload: vec![],
                        fact: when_false,
                        body: block(vec![], Tail::Value(variant(1, reflect(false, when_false)))),
                    },
                    Arm {
                        payload: vec![],
                        fact: when_true,
                        body: block(vec![], Tail::Value(variant(0, reflect(true, when_true)))),
                    },
                ],
            },
        ),
    };
    let classify = program.declare(classify).unwrap();

    // A caller matches on the result and recovers the evidence in its arm:
    // fn zero_or_self(n: u8) -> u8 { match classify(n) { Zero(v, _) => v, NonZero(v, h) => v } }
    let (m_id, m) = var();
    let (c_id, c) = var();
    let (out_id, out) = var();
    let arm = |uses_evidence: bool| {
        let (v_id, v) = var();
        let (h_id, h) = var();
        let stmts = if uses_evidence {
            vec![Stmt::Have {
                hyp: HypId::fresh(),
                claim: u8_eq(v.clone(), Term::U8(0)),
                proof: Proof::OfTerm(h),
            }]
        } else {
            vec![]
        };
        Arm {
            payload: vec![v_id, h_id],
            fact: HypId::fresh(),
            body: block(stmts, Tail::Value(v)),
        }
    };
    let caller = ExecFn {
        signature: Type::function(1, |_| Type::U8),
        params: vec![m_id],
        body: block(
            vec![
                Stmt::Call {
                    var: c_id,
                    callee: classify,
                    arguments: vec![m],
                },
                Stmt::Match {
                    var: out_id,
                    ty: Type::U8,
                    scrutinee: c,
                    arms: vec![arm(true), arm(false)],
                },
            ],
            Tail::Value(out),
        ),
    };
    assert_eq!(program.declare(caller).map(|_| ()), Ok(()));
}

/// fn bounded_walk(limit: u8) -> (value: u8, evidence: @[value <= limit])
/// from specification section 10.4.
fn bounded_walk(world: &World, carry_the_invariant: bool) -> ExecFn {
    let prelude = world.prelude;
    let theory = world.theory;
    let (limit_id, limit) = var();
    let (i_id, i) = var();
    let (bound_id, bound) = var();
    let (next_id, next) = var();
    let (walked_id, walked) = var();
    let result = |limit: &Term| {
        let limit = limit.clone();
        Type::tuple(move |earlier| match earlier {
            [] => Some(Type::U8),
            [value] => Some(Type::proof(
                prelude.u8_le_prop(value.clone(), limit.clone()),
            )),
            _ => None,
        })
    };
    // (i: u8, bound: @[i <= limit])
    let state = result(&limit);
    let comparison = Term::prim(Prim::U8Eq, vec![i.clone(), limit.clone()]);
    let (when_false, when_true) = (HypId::fresh(), HypId::fresh());
    let (differs, below, next_is, next_bound) = (
        HypId::fresh(),
        HypId::fresh(),
        HypId::fresh(),
        HypId::fresh(),
    );
    let limit_in = limit.clone();
    let carried = if carry_the_invariant {
        Proof::hyp(next_bound)
    } else {
        // The old evidence is about the old i.
        Proof::OfTerm(bound.clone())
    };
    let keep_walking = block(
        vec![
            Stmt::Have {
                hyp: differs,
                claim: prelude.not_prop(u8_eq(i.clone(), limit.clone())),
                proof: Proof::implies_elim(
                    Proof::Axiom(Axiom::Reflect(comparison.clone(), false)),
                    Proof::hyp(when_false),
                ),
            },
            Stmt::Have {
                hyp: below,
                claim: prelude.u8_lt_prop(i.clone(), limit.clone()),
                proof: lemma(
                    theory.u8_lt_of_le_of_ne,
                    vec![
                        i.clone(),
                        limit.clone(),
                        Term::proof(Proof::OfTerm(bound.clone())),
                        Term::proof(Proof::hyp(differs)),
                    ],
                ),
            },
            Stmt::Let {
                var: next_id,
                equation: next_is,
                value: add_one(i.clone()),
            },
            Stmt::Have {
                hyp: next_bound,
                claim: prelude.u8_le_prop(next.clone(), limit.clone()),
                proof: Proof::transport(
                    symm_at(&Type::U8, &next, Proof::hyp(next_is)),
                    |hole| prelude.u8_le_prop(hole, limit_in.clone()),
                    lemma(
                        theory.u8_succ_le_of_lt,
                        vec![i.clone(), limit.clone(), Term::proof(Proof::hyp(below))],
                    ),
                ),
            },
        ],
        Tail::Continue(vec![next, Term::proof(carried)]),
    );
    let stop = block(
        vec![],
        Tail::Break(Term::tuple(
            &result(&limit),
            vec![i.clone(), Term::proof(Proof::OfTerm(bound))],
        )),
    );
    ExecFn {
        signature: Type::function(1, |params| match params {
            [] => Type::U8,
            [limit] => result(limit),
            _ => unreachable!(),
        }),
        params: vec![limit_id],
        body: block(
            vec![Stmt::Loop {
                var: walked_id,
                state,
                vars: vec![i_id, bound_id],
                init: vec![
                    Term::U8(0),
                    Term::proof(lemma(theory.u8_zero_le, vec![limit.clone()])),
                ],
                result: result(&limit),
                body: block(
                    vec![],
                    Tail::Match {
                        scrutinee: comparison,
                        arms: vec![
                            Arm {
                                payload: vec![],
                                fact: when_false,
                                body: keep_walking,
                            },
                            Arm {
                                payload: vec![],
                                fact: when_true,
                                body: stop,
                            },
                        ],
                    },
                ),
            }],
            Tail::Value(walked),
        ),
    }
}

#[test]
fn a_loop_invariant_is_state_evidence() {
    let world = world();
    let mut program = Program::new(Rc::clone(&world.definitions));
    assert_eq!(
        program.declare(bounded_walk(&world, true)).map(|_| ()),
        Ok(())
    );
    // Continuing without re-establishing the state's proof field.
    assert!(matches!(
        program.declare(bounded_walk(&world, false)),
        Err(ExecError::Kernel(KernelError::ProofMismatch { .. }))
    ));
}

/// fn spin() -> @[false] { loop () -> @[false] { continue(); } }
fn spin(prelude: &Prelude) -> ExecFn {
    let (never_id, never) = var();
    let falsehood = Type::proof(prelude.falsehood_prop());
    ExecFn {
        signature: Type::function(0, |_| falsehood.clone()),
        params: vec![],
        body: block(
            vec![Stmt::Loop {
                var: never_id,
                state: Type::Tuple(vec![]),
                vars: vec![],
                init: vec![],
                result: falsehood.clone(),
                body: block(vec![], Tail::Continue(vec![])),
            }],
            Tail::Value(never),
        ),
    }
}

#[test]
fn a_divergent_function_may_advertise_a_proof_of_false() {
    // Partial correctness: spin is accepted, and so is a caller that relies
    // on what spin returns, because that code is never reached. Nothing here
    // becomes a theorem: a kernel term has no way to name an ordinary
    // function, so no math function or lemma can call spin.
    let world = world();
    let mut program = Program::new(Rc::clone(&world.definitions));
    let spin = program.declare(spin(&world.prelude)).unwrap();

    let (impossible_id, impossible) = var();
    let caller = ExecFn {
        signature: Type::function(0, |_| Type::U8),
        params: vec![],
        body: block(
            vec![Stmt::Call {
                var: impossible_id,
                callee: spin,
                arguments: vec![],
            }],
            // Any value at all, from the proof that is never produced.
            Tail::Value(Term::Absurd(Box::new(Proof::OfTerm(impossible)), Type::U8)),
        ),
    };
    assert_eq!(program.declare(caller).map(|_| ()), Ok(()));
}

// --- Rejected programs -------------------------------------------------------

fn returns_u8(params: Vec<VarId>, arity: usize, body: Block) -> ExecFn {
    ExecFn {
        signature: Type::function(arity, |_| Type::U8),
        params,
        body,
    }
}

#[test]
fn control_flow_is_checked() {
    let world = world();
    let mut program = Program::new(world.definitions);
    let (out_id, out) = var();

    // A loop body that produces a value instead of breaking or continuing.
    let falls_through = returns_u8(
        vec![],
        0,
        block(
            vec![Stmt::Loop {
                var: out_id,
                state: Type::Tuple(vec![]),
                vars: vec![],
                init: vec![],
                result: Type::U8,
                body: block(vec![], Tail::Value(Term::U8(1))),
            }],
            Tail::Value(out),
        ),
    );
    assert_eq!(
        program.declare(falls_through).map(|_| ()),
        Err(ExecError::FallsThrough)
    );

    let stray_break = returns_u8(vec![], 0, block(vec![], Tail::Break(Term::U8(1))));
    assert_eq!(
        program.declare(stray_break).map(|_| ()),
        Err(ExecError::NoEnclosingLoop)
    );

    // A break must carry the loop's result type.
    let (r_id, r) = var();
    let wrong_break = returns_u8(
        vec![],
        0,
        block(
            vec![Stmt::Loop {
                var: r_id,
                state: Type::Tuple(vec![]),
                vars: vec![],
                init: vec![],
                result: Type::U8,
                body: block(vec![], Tail::Break(Term::Bool(true))),
            }],
            Tail::Value(r),
        ),
    );
    assert!(matches!(
        program.declare(wrong_break),
        Err(ExecError::Kernel(KernelError::TypeMismatch { .. }))
    ));

    // One arm too few.
    let (b_id, b) = var();
    let short_match = ExecFn {
        signature: Type::function(1, |params| match params {
            [] => Type::Bool,
            _ => Type::U8,
        }),
        params: vec![b_id],
        body: block(
            vec![],
            Tail::Match {
                scrutinee: b,
                arms: vec![Arm {
                    payload: vec![],
                    fact: HypId::fresh(),
                    body: block(vec![], Tail::Value(Term::U8(0))),
                }],
            },
        ),
    };
    assert_eq!(
        program.declare(short_match).map(|_| ()),
        Err(ExecError::BadMatch)
    );
}

#[test]
fn a_ghost_cannot_reach_executable_data_or_control() {
    let world = world();
    let mut program = Program::new(world.definitions);
    // let k = of_nat(to_nat(n)) is a u8 that only logic can compute: to_nat
    // has no runtime form. So k is ghost.
    let ghost_let = |k: VarId, n: &Term| Stmt::Let {
        var: k,
        equation: HypId::fresh(),
        value: Term::of_nat(Term::to_nat(n.clone())),
    };

    let (n_id, n) = var();
    let (k_id, k) = var();
    let returns_ghost = returns_u8(
        vec![n_id],
        1,
        block(vec![ghost_let(k_id, &n)], Tail::Value(k)),
    );
    assert!(matches!(
        program.declare(returns_ghost),
        Err(ExecError::Kernel(KernelError::GhostInExecutable(_)))
    ));

    let (n_id, n) = var();
    let (k_id, k) = var();
    let arm = |value: u8| Arm {
        payload: vec![],
        fact: HypId::fresh(),
        body: block(vec![], Tail::Value(Term::U8(value))),
    };
    let branches_on_ghost = returns_u8(
        vec![n_id],
        1,
        block(
            vec![ghost_let(k_id, &n)],
            Tail::Match {
                scrutinee: Term::prim(Prim::U8Eq, vec![k, Term::U8(0)]),
                arms: vec![arm(0), arm(1)],
            },
        ),
    );
    assert!(matches!(
        program.declare(branches_on_ghost),
        Err(ExecError::Kernel(KernelError::GhostInExecutable(_)))
    ));

    // The same ghost is welcome in a proposition.
    let (n_id, n) = var();
    let (k_id, k) = var();
    let mentions_ghost = returns_u8(
        vec![n_id],
        1,
        block(
            vec![
                ghost_let(k_id, &n),
                Stmt::Have {
                    hyp: HypId::fresh(),
                    claim: u8_eq(k.clone(), k.clone()),
                    proof: Proof::Refl(k),
                },
            ],
            Tail::Value(n),
        ),
    );
    assert!(program.declare(mentions_ghost).is_ok());
}

#[test]
fn calls_and_bindings_are_checked() {
    let world = world();
    let mut program = Program::new(Rc::clone(&world.definitions));

    // fn needs_three(n: u8, h: @[n == 3]) -> u8 { n }
    let (n_id, n) = var();
    let h_id = VarId::fresh();
    let needs_three = ExecFn {
        signature: Type::function(2, |params| match params {
            [] => Type::U8,
            [n] => Type::proof(u8_eq(n.clone(), Term::U8(3))),
            _ => Type::U8,
        }),
        params: vec![n_id, h_id],
        body: block(vec![], Tail::Value(n)),
    };
    let needs_three = program.declare(needs_three).unwrap();

    let call_with = |argument: Term, proof: Proof, callee: ExecFnId| {
        let (r_id, r) = var();
        returns_u8(
            vec![],
            0,
            block(
                vec![Stmt::Call {
                    var: r_id,
                    callee,
                    arguments: vec![argument, Term::proof(proof)],
                }],
                Tail::Value(r),
            ),
        )
    };
    assert!(
        program
            .declare(call_with(
                Term::U8(3),
                Proof::Refl(Term::U8(3)),
                needs_three
            ))
            .is_ok()
    );
    // The precondition is a proof obligation at the call.
    assert!(matches!(
        program.declare(call_with(
            Term::U8(4),
            Proof::Refl(Term::U8(4)),
            needs_three
        )),
        Err(ExecError::Kernel(KernelError::ProofMismatch { .. }))
    ));
    // A function cannot be called before it exists, so it cannot call itself.
    let later = {
        let mut bigger = Program::new(Rc::clone(&world.definitions));
        for _ in 0..3 {
            bigger.declare(increment()).unwrap();
        }
        bigger.declare(increment()).unwrap()
    };
    assert_eq!(
        program
            .declare(call_with(Term::U8(3), Proof::Refl(Term::U8(3)), later))
            .map(|_| ()),
        Err(ExecError::UnknownFunction)
    );

    // An identity is bound once.
    let (x_id, x) = var();
    let rebinds = returns_u8(
        vec![],
        0,
        block(
            vec![
                Stmt::Let {
                    var: x_id,
                    equation: HypId::fresh(),
                    value: Term::U8(1),
                },
                Stmt::Let {
                    var: x_id,
                    equation: HypId::fresh(),
                    value: Term::U8(2),
                },
            ],
            Tail::Value(x),
        ),
    );
    assert_eq!(
        program.declare(rebinds).map(|_| ()),
        Err(ExecError::Kernel(KernelError::DuplicateBinding))
    );

    // A binding made in one arm is not in scope after the match.
    let (b_id, b) = var();
    let (inner_id, inner) = var();
    let (m_id, _) = var();
    let arm = || Arm {
        payload: vec![],
        fact: HypId::fresh(),
        body: block(
            vec![Stmt::Let {
                var: inner_id,
                equation: HypId::fresh(),
                value: Term::U8(1),
            }],
            Tail::Value(Term::U8(0)),
        ),
    };
    let escapes = ExecFn {
        signature: Type::function(1, |params| match params {
            [] => Type::Bool,
            _ => Type::U8,
        }),
        params: vec![b_id],
        body: block(
            vec![Stmt::Match {
                var: m_id,
                ty: Type::U8,
                scrutinee: b,
                arms: vec![arm(), arm()],
            }],
            Tail::Value(inner),
        ),
    };
    assert!(matches!(
        program.declare(escapes),
        Err(ExecError::Kernel(KernelError::UnknownVariable(_)))
    ));
}

// --- Bounded for ----------------------------------------------------------------

/// The state `(acc: u8, same: @[acc == i])` as a function of the index.
fn counting_state() -> Type {
    Type::function(1, |params| match params {
        [] => Type::U8,
        [i] => {
            let i = i.clone();
            Type::tuple(move |earlier| match earlier {
                [] => Some(Type::U8),
                [acc] => Some(Type::proof(u8_eq(acc.clone(), i.clone()))),
                _ => None,
            })
        }
        _ => unreachable!(),
    })
}

fn counted_result(n: &Term) -> Type {
    let n = n.clone();
    Type::tuple(move |earlier| match earlier {
        [] => Some(Type::U8),
        [total] => Some(Type::proof(u8_eq(total.clone(), n.clone()))),
        _ => None,
    })
}

#[derive(Clone, Copy, PartialEq)]
enum CountBug {
    None,
    StaleInvariant,
    Breaks,
    FallsThrough,
    Unordered,
}

/// fn count_by_calls(n: u8) -> (total: u8, same: @[total == n]) {
///     for i in 0..n (acc: u8 = 0, same: @[acc == i] = _) {
///         let r = increment(acc);          // an ordinary call in the body
///         continue(r.0, _);                // r.0 == acc + 1 == i + 1
///     }
/// }
fn count_by_calls(world: &World, increment: ExecFnId, bug: CountBug) -> ExecFn {
    let (n_id, n) = var();
    let (i_id, i) = var();
    let (acc_id, acc) = var();
    let (same_id, same) = var();
    let (r_id, r) = var();
    let (done_id, done) = var();
    let advanced = HypId::fresh();
    let stepped = Term::proj(r.clone(), 0);
    // r.1 : r.0 == acc + 1, and same : acc == i, give r.0 == i + 1.
    let left = stepped.clone();
    let proof = Proof::transport(
        Proof::OfTerm(same.clone()),
        |hole| u8_eq(left.clone(), add_one(hole)),
        Proof::OfTerm(Term::proj(r, 1)),
    );
    let tail = match bug {
        CountBug::Breaks => Tail::Break(Term::U8(0)),
        CountBug::FallsThrough => Tail::Value(Term::U8(0)),
        CountBug::StaleInvariant => {
            Tail::Continue(vec![stepped.clone(), Term::proof(Proof::OfTerm(same))])
        }
        _ => Tail::Continue(vec![stepped.clone(), Term::proof(Proof::hyp(advanced))]),
    };
    let ordered = if bug == CountBug::Unordered {
        // A true fact, about the wrong bounds.
        lemma(world.theory.u8_le_refl, vec![n.clone()])
    } else {
        lemma(world.theory.u8_zero_le, vec![n.clone()])
    };
    ExecFn {
        signature: Type::function(1, |params| match params {
            [] => Type::U8,
            [n] => counted_result(n),
            _ => unreachable!(),
        }),
        params: vec![n_id],
        body: block(
            vec![Stmt::For(Box::new(ForStmt {
                var: done_id,
                index: i_id,
                lower: HypId::fresh(),
                upper: HypId::fresh(),
                lo: Term::U8(0),
                hi: n,
                ordered,
                state: counting_state(),
                vars: vec![acc_id, same_id],
                init: vec![Term::U8(0), Term::proof(Proof::Refl(Term::U8(0)))],
                body: block(
                    vec![
                        Stmt::Call {
                            var: r_id,
                            callee: increment,
                            arguments: vec![acc],
                        },
                        Stmt::Have {
                            hyp: advanced,
                            claim: u8_eq(stepped, add_one(i)),
                            proof,
                        },
                    ],
                    tail,
                ),
            }))],
            // The state at the final index n is exactly the declared result.
            Tail::Value(done),
        ),
    }
}

#[test]
fn a_bounded_for_may_call_ordinary_functions_and_carries_an_indexed_invariant() {
    let world = world();
    let mut program = Program::new(Rc::clone(&world.definitions));
    let increment = program.declare(increment()).unwrap();
    assert_eq!(
        program
            .declare(count_by_calls(&world, increment, CountBug::None))
            .map(|_| ()),
        Ok(())
    );
    // The invariant is about the index, so the old evidence does not carry.
    assert!(matches!(
        program.declare(count_by_calls(&world, increment, CountBug::StaleInvariant)),
        Err(ExecError::Kernel(KernelError::ProofMismatch { .. }))
    ));
    assert_eq!(
        program
            .declare(count_by_calls(&world, increment, CountBug::Breaks))
            .map(|_| ()),
        Err(ExecError::BreakInFor)
    );
    assert_eq!(
        program
            .declare(count_by_calls(&world, increment, CountBug::FallsThrough))
            .map(|_| ()),
        Err(ExecError::FallsThrough)
    );
    // The bounds must be shown to be ordered, by a proof about these bounds.
    assert!(matches!(
        program.declare(count_by_calls(&world, increment, CountBug::Unordered)),
        Err(ExecError::Kernel(KernelError::ProofMismatch { .. }))
    ));
}

#[test]
fn a_for_inside_a_loop_takes_the_continue_and_refuses_the_break() {
    // fn twice_over(n: u8) -> u8 {
    //     loop () -> u8 {
    //         let swept = for i in 0..n (last: u8 = 0) { continue(i) };
    //         break swept.0;
    //     }
    // }
    let world = world();
    let mut program = Program::new(Rc::clone(&world.definitions));
    let plain_state = || {
        Type::function(1, |params| match params {
            [] => Type::U8,
            _ => Type::Tuple(vec![Type::U8]),
        })
    };
    let build = |inner_tail: fn(Term) -> Tail| {
        let (n_id, n) = var();
        let (i_id, i) = var();
        let (swept_id, swept) = var();
        let (out_id, out) = var();
        returns_u8(
            vec![n_id],
            1,
            block(
                vec![Stmt::Loop {
                    var: out_id,
                    state: Type::Tuple(vec![]),
                    vars: vec![],
                    init: vec![],
                    result: Type::U8,
                    body: block(
                        vec![Stmt::For(Box::new(ForStmt {
                            var: swept_id,
                            index: i_id,
                            lower: HypId::fresh(),
                            upper: HypId::fresh(),
                            lo: Term::U8(0),
                            hi: n.clone(),
                            ordered: lemma(world.theory.u8_zero_le, vec![n]),
                            state: plain_state(),
                            vars: vec![VarId::fresh()],
                            init: vec![Term::U8(0)],
                            body: block(vec![], inner_tail(i)),
                        }))],
                        Tail::Break(Term::proj(swept, 0)),
                    ),
                }],
                Tail::Value(out),
            ),
        )
    };
    // Inside the for, continue belongs to the for: it takes the for's state.
    assert!(program.declare(build(|i| Tail::Continue(vec![i]))).is_ok());
    // The loop's own continue takes no arguments, so this is not it.
    assert!(program.declare(build(|_| Tail::Continue(vec![]))).is_err());
    // And break does not reach past the for to the loop.
    assert_eq!(
        program.declare(build(Tail::Break)).map(|_| ()),
        Err(ExecError::BreakInFor)
    );
}

#[test]
fn a_for_checks_its_bounds_and_state_shape() {
    let world = world();
    let mut program = Program::new(Rc::clone(&world.definitions));
    let build = |hi_is_ghost: bool, state: Type| {
        let (n_id, n) = var();
        let (k_id, k) = var();
        let (done_id, done) = var();
        let hi = if hi_is_ghost { k.clone() } else { n.clone() };
        returns_u8(
            vec![n_id],
            1,
            block(
                vec![
                    // k is a u8 only logic can compute, hence ghost.
                    Stmt::Let {
                        var: k_id,
                        equation: HypId::fresh(),
                        value: Term::of_nat(Term::to_nat(n)),
                    },
                    Stmt::For(Box::new(ForStmt {
                        var: done_id,
                        index: VarId::fresh(),
                        lower: HypId::fresh(),
                        upper: HypId::fresh(),
                        lo: Term::U8(0),
                        hi: hi.clone(),
                        ordered: lemma(world.theory.u8_zero_le, vec![hi]),
                        state,
                        vars: vec![VarId::fresh()],
                        init: vec![Term::U8(0)],
                        body: block(vec![], Tail::Continue(vec![Term::U8(0)])),
                    })),
                ],
                Tail::Value(Term::proj(done, 0)),
            ),
        )
    };
    let plain_state = || {
        Type::function(1, |params| match params {
            [] => Type::U8,
            _ => Type::Tuple(vec![Type::U8]),
        })
    };
    assert!(program.declare(build(false, plain_state())).is_ok());
    // A ghost cannot decide how many times executable code runs.
    assert!(matches!(
        program.declare(build(true, plain_state())),
        Err(ExecError::Kernel(KernelError::GhostInExecutable(_)))
    ));
    // The state must be a function from the index to a tuple type.
    assert_eq!(
        program
            .declare(build(false, Type::Tuple(vec![Type::U8])))
            .map(|_| ()),
        Err(ExecError::BadLoopState)
    );
    let not_a_tuple = Type::function(1, |_| Type::U8);
    assert_eq!(
        program.declare(build(false, not_a_tuple)).map(|_| ()),
        Err(ExecError::BadLoopState)
    );
}
