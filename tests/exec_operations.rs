//! The primitive operations of the check IR, `Stmt::Operate` (build task
//! E6; the Architecture in atlas.html), on hand-built programs: what the
//! trusted checker demands at an operator, under `no_panic` and without
//! it, and what the code after it may rely on.
//!
//! The rule under test: `let v = op[T](xs)` defines `v` by the equation
//! `v == op[T](xs)`, the wrapped meaning; with evidence `fits` for the
//! premises of `Row::fits`, checked one by one, the exact result `view(v)
//! == e` is also known for `+`, `-`, `*`, and unary minus, under the
//! learned identity; under `no_panic` the evidence is required at every
//! row that can panic; and after `/` or `%` the premises themselves are
//! known, in every function, because the division panics in every build
//! where they fail.

use std::rc::Rc;

use locus::erased::{Outcome, Overflow, Value};
use locus::exec::{
    Block, CheckInterpreter, ExecError, ExecFn, ExecFnId, OperateStmt, Program, Promises, Stmt,
    Tail,
};
use locus::kernel::derive::symm_at;
use locus::kernel::theory::{self, Theory};
use locus::kernel::{
    Axiom, Definitions, HypId, KernelError, MachineInt, Op, Prelude, Proof, Term, Type, VarId,
};
use locus::typed::FnRef;

const FUEL: u64 = 10_000;

struct World {
    definitions: Rc<Definitions>,
    prelude: Prelude,
    #[allow(dead_code)]
    theory: Theory,
}

fn world() -> World {
    let (mut definitions, prelude) = Definitions::with_prelude();
    let theory = theory::declare(&mut definitions, &prelude).expect("the theory checks");
    World {
        definitions: Rc::new(definitions),
        prelude,
        theory,
    }
}

fn program(world: &World) -> Program {
    Program::new((*world.definitions).clone())
}

fn var() -> (VarId, Term) {
    let id = VarId::fresh();
    (id, Term::var(id))
}

fn no_panic() -> Promises {
    Promises {
        no_panic: true,
        ..Promises::default()
    }
}

fn u8_lit(value: u8) -> Term {
    Term::machine_int(MachineInt::U8, i128::from(value))
}

fn view(ty: MachineInt, x: Term) -> Term {
    Term::view(ty, x)
}

/// `let var = op[ty](arguments)` with the given evidence and learned
/// identities.
fn operate(
    var: VarId,
    equation: HypId,
    op: Op,
    ty: MachineInt,
    arguments: Vec<Term>,
    fits: Option<Vec<Proof>>,
    learned: Vec<HypId>,
) -> Stmt {
    Stmt::Operate(Box::new(OperateStmt {
        var,
        equation,
        op,
        ty,
        arguments,
        fits,
        learned,
    }))
}

/// `fn f(n: u8) -> result { body }` with the given promises.
fn u8_fn(promises: Promises, n: VarId, result: Type, body: Block) -> ExecFn {
    ExecFn {
        promises,
        signature: Type::function(1, move |params| {
            if params.is_empty() {
                Type::U8
            } else {
                result.clone()
            }
        }),
        params: vec![n],
        body,
    }
}

/// `fn f(a: u8, b: u8) -> result { body }` with the given promises.
fn u8_pair_fn(promises: Promises, a: VarId, b: VarId, result: Type, body: Block) -> ExecFn {
    ExecFn {
        promises,
        signature: Type::function(2, move |params| {
            if params.len() < 2 {
                Type::U8
            } else {
                result.clone()
            }
        }),
        params: vec![a, b],
        body,
    }
}

/// `(m: u8, @[view(m) == view(n) + view(1)])`
fn exact_successor(n: &Term) -> Type {
    let n = n.clone();
    Type::tuple(move |earlier| match earlier {
        [] => Some(Type::U8),
        [m] => Some(Type::proof(Term::eq(
            Type::Int,
            view(MachineInt::U8, m.clone()),
            Term::int_add(
                view(MachineInt::U8, n.clone()),
                view(MachineInt::U8, u8_lit(1)),
            ),
        ))),
        _ => None,
    })
}

fn run(program: &Program, id: ExecFnId, arguments: Vec<Value>, mode: Overflow) -> Outcome {
    CheckInterpreter::new(program, FUEL)
        .with_overflow(mode)
        .call(FnRef::Exec(id), arguments)
        .expect("the program runs")
}

/// The two premises of `add[u8](n, 1)` at `n`, proved by the linear rule
/// from a hypothesis `view(n) + 1 <= 255` and the range of the view.
fn successor_fits(n: &Term, bounded: HypId) -> Vec<Proof> {
    let exact = Term::int_add(
        view(MachineInt::U8, n.clone()),
        view(MachineInt::U8, u8_lit(1)),
    );
    let lower = Proof::Linear {
        goal: Term::int_le(Term::int(0), exact.clone()),
        goal_coefficient: locus::kernel::Integer::from(1i64),
        pairs: vec![
            (
                Proof::Axiom(Axiom::ViewLower(MachineInt::U8, n.clone())),
                locus::kernel::Integer::from(1i64),
            ),
            (
                Proof::Axiom(Axiom::ViewLower(MachineInt::U8, u8_lit(1))),
                locus::kernel::Integer::from(1i64),
            ),
        ],
    };
    // `view(1) == 1` by evaluation turns the hypothesis into the premise.
    let one = view(MachineInt::U8, u8_lit(1));
    let upper = Proof::Transport {
        eq: Box::new(symm_at(&Type::Int, &one, Proof::Literal(one.clone()))),
        template: Term::int_le(
            Term::int_add(view(MachineInt::U8, n.clone()), Term::Bound(0)),
            Term::int(255),
        ),
        proof: Box::new(Proof::hyp(bounded)),
    };
    vec![lower, upper]
}

// --- Under no_panic ------------------------------------------------------------------

/// #[no_panic] fn next(n: u8) -> u8 { let m = n + 1; m }
/// Without evidence the checker refuses the operator; the same body is
/// accepted in a function that promises nothing.
#[test]
fn an_operator_under_no_panic_needs_its_evidence() {
    let world = world();
    let (n_id, n) = var();
    let (m_id, m) = var();
    let body = |learned: Vec<HypId>| Block {
        stmts: vec![operate(
            m_id,
            HypId::fresh(),
            Op::Add,
            MachineInt::U8,
            vec![n.clone(), u8_lit(1)],
            None,
            learned,
        )],
        tail: Tail::Value(m.clone()),
    };
    let mut refused = program(&world);
    assert_eq!(
        refused.declare(u8_fn(no_panic(), n_id, Type::U8, body(Vec::new()))),
        Err(ExecError::OperationUnderNoPanic {
            op: Op::Add,
            ty: MachineInt::U8
        })
    );
    let mut accepted = program(&world);
    let id = accepted
        .declare(u8_fn(Promises::default(), n_id, Type::U8, body(Vec::new())))
        .unwrap();
    // Nothing is learned without evidence: a learned identity is refused.
    let mut too_much = program(&world);
    assert_eq!(
        too_much.declare(u8_fn(
            Promises::default(),
            n_id,
            Type::U8,
            body(vec![HypId::fresh()])
        )),
        Err(ExecError::BadOperation {
            op: Op::Add,
            ty: MachineInt::U8
        })
    );
    // And the accepted one runs as Rust does, in each build.
    assert_eq!(
        run(&accepted, id, vec![Value::u8(41)], Overflow::Checks),
        Outcome::Value(Value::u8(42))
    );
    assert_eq!(
        run(&accepted, id, vec![Value::u8(255)], Overflow::Checks),
        Outcome::Panic("attempt to add with overflow".into())
    );
    assert_eq!(
        run(&accepted, id, vec![Value::u8(255)], Overflow::Wrap),
        Outcome::Value(Value::u8(0))
    );
}

/// #[no_panic] fn next(n: u8, bounded: @[view(n) + 1 <= 255])
///     -> (m: u8, @[view(m) == view(n) + view(1)]) {
///     let m = n + 1;          // with evidence of the two premises
///     (m, learned)
/// }
/// With the right evidence the exact fact is usable afterwards; with a
/// wrong proof for a premise the checker refuses the statement; and with
/// the evidence a `learned` identity is required, one for the exact fact.
#[test]
fn the_right_evidence_makes_the_exact_result_known() {
    let world = world();
    let (n_id, n) = var();
    let (m_id, m) = var();
    let bounded = HypId::fresh();
    let exact = HypId::fresh();
    let signature = {
        let n_term = n.clone();
        Type::function(2, move |params| match params {
            [] => Type::U8,
            [n_param] => Type::proof(Term::int_le(
                Term::int_add(view(MachineInt::U8, n_param.clone()), Term::int(1)),
                Term::int(255),
            )),
            [n_param, _] => exact_successor(n_param).clone(),
            _ => unreachable!("{n_term}"),
        })
    };
    let function = |fits: Vec<Proof>, learned: Vec<HypId>| ExecFn {
        promises: no_panic(),
        signature: signature.clone(),
        params: vec![n_id, VarId::fresh()],
        body: Block {
            stmts: vec![
                // The parameter's evidence as a hypothesis, to name it.
                Stmt::Have {
                    hyp: bounded,
                    claim: Term::int_le(
                        Term::int_add(view(MachineInt::U8, n.clone()), Term::int(1)),
                        Term::int(255),
                    ),
                    proof: Proof::OfTerm(Term::var(VarId::fresh())),
                },
                operate(
                    m_id,
                    HypId::fresh(),
                    Op::Add,
                    MachineInt::U8,
                    vec![n.clone(), u8_lit(1)],
                    Some(fits),
                    learned,
                ),
            ],
            tail: Tail::Value(Term::tuple(
                &exact_successor(&n),
                vec![m.clone(), Term::proof(Proof::hyp(exact))],
            )),
        },
    };
    // The parameter's identity must be the one the `Have` projects; build
    // the function with it.
    let with = |fits: Vec<Proof>, learned: Vec<HypId>| {
        let mut function = function(fits, learned);
        let evidence = function.params[1];
        if let Stmt::Have { proof, .. } = &mut function.body.stmts[0] {
            *proof = Proof::OfTerm(Term::var(evidence));
        }
        function
    };

    let mut accepted = program(&world);
    let id = accepted
        .declare(with(successor_fits(&n, bounded), vec![exact]))
        .unwrap();
    assert_eq!(
        run(
            &accepted,
            id,
            vec![Value::u8(41), Value::Proved],
            Overflow::Checks
        ),
        Outcome::Value(Value::Tuple(vec![Value::u8(42), Value::Proved]))
    );

    // The premises exchanged: each proof is checked against its own.
    let mut exchanged = successor_fits(&n, bounded);
    exchanged.swap(0, 1);
    let mut refused = program(&world);
    assert!(matches!(
        refused.declare(with(exchanged, vec![exact])),
        Err(ExecError::Kernel(KernelError::ProofMismatch { .. }))
    ));

    // One proof where the row has two premises.
    let mut short = successor_fits(&n, bounded);
    short.pop();
    let mut refused = program(&world);
    assert_eq!(
        refused.declare(with(short, vec![exact])),
        Err(ExecError::BadOperation {
            op: Op::Add,
            ty: MachineInt::U8
        })
    );

    // With the evidence, exactly one learned identity, for the exact fact.
    let mut refused = program(&world);
    assert_eq!(
        refused.declare(with(successor_fits(&n, bounded), Vec::new())),
        Err(ExecError::BadOperation {
            op: Op::Add,
            ty: MachineInt::U8
        })
    );

    // Without the promise the evidence is still checked and still teaches
    // the exact fact.
    let mut unpromised = with(successor_fits(&n, bounded), vec![exact]);
    unpromised.promises = Promises::default();
    let mut accepted = program(&world);
    assert!(accepted.declare(unpromised).is_ok());
}

/// The exact fact is `view(m) == e` about the statement's own variable: a
/// claim about another variable, or with the operands exchanged, is not
/// what was learned.
#[test]
fn the_exact_fact_is_about_the_result_and_the_operands_as_written() {
    let world = world();
    let (n_id, n) = var();
    let (m_id, m) = var();
    let evidence = VarId::fresh();
    let bounded = HypId::fresh();
    let exact = HypId::fresh();
    let claim = |left: Term, right: Term| {
        Term::eq(
            Type::Int,
            view(MachineInt::U8, left),
            Term::int_add(view(MachineInt::U8, right), view(MachineInt::U8, u8_lit(1))),
        )
    };
    // fn f(n: u8, evidence: @[view(n) + 1 <= 255]) -> u8
    let with = |claimed: Term| ExecFn {
        promises: Promises::default(),
        signature: Type::function(2, |params| match params {
            [] => Type::U8,
            [n_param] => Type::proof(Term::int_le(
                Term::int_add(view(MachineInt::U8, n_param.clone()), Term::int(1)),
                Term::int(255),
            )),
            _ => Type::U8,
        }),
        params: vec![n_id, evidence],
        body: Block {
            stmts: vec![
                Stmt::Have {
                    hyp: bounded,
                    claim: Term::int_le(
                        Term::int_add(view(MachineInt::U8, n.clone()), Term::int(1)),
                        Term::int(255),
                    ),
                    proof: Proof::OfTerm(Term::var(evidence)),
                },
                operate(
                    m_id,
                    HypId::fresh(),
                    Op::Add,
                    MachineInt::U8,
                    vec![n.clone(), u8_lit(1)],
                    Some(successor_fits(&n, bounded)),
                    vec![exact],
                ),
                Stmt::Have {
                    hyp: HypId::fresh(),
                    claim: claimed,
                    proof: Proof::hyp(exact),
                },
            ],
            tail: Tail::Value(m.clone()),
        },
    };
    let mut accepted = program(&world);
    assert!(accepted.declare(with(claim(m.clone(), n.clone()))).is_ok());
    let mut refused = program(&world);
    assert!(matches!(
        refused.declare(with(claim(n.clone(), n.clone()))),
        Err(ExecError::Kernel(KernelError::ProofMismatch { .. }))
    ));
    let mut refused = program(&world);
    assert!(matches!(
        refused.declare(with(claim(m.clone(), m.clone()))),
        Err(ExecError::Kernel(KernelError::ProofMismatch { .. }))
    ));
}

// --- Division ------------------------------------------------------------------------

/// fn quotient(a: u8, b: u8) -> u8 { let q = a / b; q }
/// After the statement the premise `view(b) != 0` is known, in a function
/// that promises nothing; a learned identity is required for it, and a
/// claim it does not make is refused.
#[test]
fn a_division_teaches_its_condition_to_what_follows() {
    let world = world();
    let (a_id, a) = var();
    let (b_id, b) = var();
    let (q_id, q) = var();
    let nonzero = HypId::fresh();
    let not_zero = |x: Term| {
        world
            .prelude
            .not_prop(Term::eq(Type::Int, view(MachineInt::U8, x), Term::int(0)))
    };
    let function = |learned: Vec<HypId>, claimed: Option<Term>| {
        let mut stmts = vec![operate(
            q_id,
            HypId::fresh(),
            Op::Div,
            MachineInt::U8,
            vec![a.clone(), b.clone()],
            None,
            learned,
        )];
        if let Some(claim) = claimed {
            stmts.push(Stmt::Have {
                hyp: HypId::fresh(),
                claim,
                proof: Proof::hyp(nonzero),
            });
        }
        u8_pair_fn(
            Promises::default(),
            a_id,
            b_id,
            Type::U8,
            Block {
                stmts,
                tail: Tail::Value(q.clone()),
            },
        )
    };
    let mut accepted = program(&world);
    let id = accepted
        .declare(function(vec![nonzero], Some(not_zero(b.clone()))))
        .unwrap();
    for mode in Overflow::ALL {
        assert_eq!(
            run(&accepted, id, vec![Value::u8(7), Value::u8(2)], mode),
            Outcome::Value(Value::u8(3))
        );
        assert_eq!(
            run(&accepted, id, vec![Value::u8(7), Value::u8(0)], mode),
            Outcome::Panic("attempt to divide by zero".into())
        );
    }
    // The dividend is not what the condition speaks of.
    let mut refused = program(&world);
    assert!(matches!(
        refused.declare(function(vec![nonzero], Some(not_zero(a.clone())))),
        Err(ExecError::Kernel(KernelError::ProofMismatch { .. }))
    ));
    // One learned identity per premise: none, or two at an unsigned type,
    // is the wrong shape.
    for learned in [Vec::new(), vec![nonzero, HypId::fresh()]] {
        let mut refused = program(&world);
        assert_eq!(
            refused.declare(function(learned, None)),
            Err(ExecError::BadOperation {
                op: Op::Div,
                ty: MachineInt::U8
            })
        );
    }
    // Under `no_panic` the same premise is an obligation.
    let mut refused = program(&world);
    let mut promised = function(vec![nonzero], None);
    promised.promises = no_panic();
    assert_eq!(
        refused.declare(promised),
        Err(ExecError::OperationUnderNoPanic {
            op: Op::Div,
            ty: MachineInt::U8
        })
    );
}

/// At a signed type a division has two premises, and both are known
/// afterwards; `min / -1` panics in the interpreter in both modes.
#[test]
fn a_signed_division_teaches_both_premises() {
    let world = world();
    let (a_id, a) = var();
    let (b_id, b) = var();
    let (q_id, q) = var();
    let learned = [HypId::fresh(), HypId::fresh()];
    let i8 = MachineInt::I8;
    let not_min_over_minus_one = Term::implies(
        Term::eq(Type::Int, view(i8, a.clone()), Term::int(-128)),
        world
            .prelude
            .not_prop(Term::eq(Type::Int, view(i8, b.clone()), Term::int(-1))),
    );
    let function = ExecFn {
        promises: Promises::default(),
        signature: Type::function(2, |_| Type::machine(i8)),
        params: vec![a_id, b_id],
        body: Block {
            stmts: vec![
                operate(
                    q_id,
                    HypId::fresh(),
                    Op::Rem,
                    i8,
                    vec![a.clone(), b.clone()],
                    None,
                    learned.to_vec(),
                ),
                Stmt::Have {
                    hyp: HypId::fresh(),
                    claim: not_min_over_minus_one,
                    proof: Proof::hyp(learned[1]),
                },
            ],
            tail: Tail::Value(q.clone()),
        },
    };
    let mut accepted = program(&world);
    let id = accepted.declare(function).unwrap();
    let byte = |value: i128| Value::Int(i8, value);
    for mode in Overflow::ALL {
        assert_eq!(
            run(&accepted, id, vec![byte(-7), byte(2)], mode),
            Outcome::Value(byte(-1))
        );
        assert_eq!(
            run(&accepted, id, vec![byte(-128), byte(-1)], mode),
            Outcome::Panic("attempt to calculate the remainder with overflow".into())
        );
    }
}

// --- The wrapped meaning ------------------------------------------------------------

/// fn sum(a: u8, b: u8) -> (s: u8, @[s == wrap(view(a) + view(b))]) {
///     let s = a + b;
///     (s, op_model moved onto s)
/// }
/// The equation of the statement gives the wrapped result in every
/// function, which is what `op_model` states of the applied row.
#[test]
fn the_wrapped_meaning_is_known_without_evidence() {
    let world = world();
    let (a_id, a) = var();
    let (b_id, b) = var();
    let (s_id, s) = var();
    let equation = HypId::fresh();
    let u8 = MachineInt::U8;
    // `(s: u8, @[s == wrap(view(a) + view(b))])` over the given `a` and `b`.
    let result_over = |a: &Term, b: &Term| {
        let meaning = Term::wrap(u8, Term::int_add(view(u8, a.clone()), view(u8, b.clone())));
        Type::tuple(move |earlier| match earlier {
            [] => Some(Type::U8),
            [s] => Some(Type::proof(Term::eq(Type::U8, s.clone(), meaning.clone()))),
            _ => None,
        })
    };
    let result = result_over(&a, &b);
    let meaning = Term::wrap(u8, Term::int_add(view(u8, a.clone()), view(u8, b.clone())));
    // s == add(a, b) and add(a, b) == wrap(e): transport the axiom along
    // the symmetric equation.
    let proof = Proof::Transport {
        eq: Box::new(symm_at(&Type::U8, &s, Proof::hyp(equation))),
        template: Term::eq(Type::U8, Term::Bound(0), meaning.clone()),
        proof: Box::new(Proof::Axiom(Axiom::OpModel(
            Op::Add,
            u8,
            vec![a.clone(), b.clone()],
        ))),
    };
    let function = ExecFn {
        promises: Promises::default(),
        signature: Type::function(2, move |params| match params {
            [a, b] => result_over(a, b),
            _ => Type::U8,
        }),
        params: vec![a_id, b_id],
        body: Block {
            stmts: vec![operate(
                s_id,
                equation,
                Op::Add,
                u8,
                vec![a.clone(), b.clone()],
                None,
                Vec::new(),
            )],
            tail: Tail::Value(Term::tuple(&result, vec![s.clone(), Term::proof(proof)])),
        },
    };
    let mut accepted = program(&world);
    let id = accepted.declare(function).unwrap();
    // The claim holds in both builds: the value the wrapping build gives
    // is the wrapped one, and the checked build panics instead of giving
    // another.
    assert_eq!(
        run(
            &accepted,
            id,
            vec![Value::u8(200), Value::u8(100)],
            Overflow::Wrap
        ),
        Outcome::Value(Value::Tuple(vec![Value::u8(44), Value::Proved]))
    );
    assert_eq!(
        run(
            &accepted,
            id,
            vec![Value::u8(200), Value::u8(100)],
            Overflow::Checks
        ),
        Outcome::Panic("attempt to add with overflow".into())
    );
}

/// A row that does not exist, or the wrong number of arguments, is refused
/// before anything is assumed.
#[test]
fn a_missing_row_or_a_wrong_arity_is_refused() {
    let world = world();
    let (n_id, n) = var();
    let (m_id, m) = var();
    let with = |op: Op, arguments: Vec<Term>| {
        u8_fn(
            Promises::default(),
            n_id,
            Type::U8,
            Block {
                stmts: vec![operate(
                    m_id,
                    HypId::fresh(),
                    op,
                    MachineInt::U8,
                    arguments,
                    None,
                    Vec::new(),
                )],
                tail: Tail::Value(m.clone()),
            },
        )
    };
    let mut refused = program(&world);
    assert_eq!(
        refused.declare(with(Op::Neg, vec![n.clone()])),
        Err(ExecError::Kernel(KernelError::NoRow(
            Op::Neg,
            MachineInt::U8
        )))
    );
    let mut refused = program(&world);
    assert_eq!(
        refused.declare(with(Op::Add, vec![n.clone()])),
        Err(ExecError::BadOperation {
            op: Op::Add,
            ty: MachineInt::U8
        })
    );
    // An argument of another type.
    let mut refused = program(&world);
    assert!(matches!(
        refused.declare(with(
            Op::Add,
            vec![n.clone(), Term::machine_int(MachineInt::U16, 1)]
        )),
        Err(ExecError::Kernel(KernelError::TypeMismatch { .. }))
    ));
}
