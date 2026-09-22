//! Acceptance tests for kernel gate K4 (the kernel contract in atlas.html): enums and case
//! analysis with arm evidence, declared propositions with index equations,
//! `Exists`, and excluded middle with dependency recording.
//! Every term here is written by hand; nothing comes from the parser.

use std::rc::Rc;

use locus::kernel::derive::{symm, trans};
use locus::kernel::{
    CmpOp, Context, Definitions, EnumId, KernelError, MachineInt, Mode, Op, Prelude, Proof, PropId,
    PropVariant, Term, Type, check_proof, infer_proof, infer_term, proof_is_classical, same,
};

fn u8_eq(left: Term, right: Term) -> Term {
    Term::eq(Type::U8, left, right)
}

fn unit() -> Type {
    Type::Tuple(vec![])
}

/// `enum Light { Red, Green }`
fn declare_light(definitions: &mut Definitions) -> EnumId {
    definitions.declare_enum(&[unit(), unit()]).unwrap()
}

fn red(light: EnumId) -> Term {
    Term::Variant(light, 0, vec![])
}

fn green(light: EnumId) -> Term {
    Term::Variant(light, 1, vec![])
}

/// `prop Small(n: u8) { Two: @Small(2), Three: @Small(3) }`
fn declare_small(definitions: &mut Definitions) -> PropId {
    definitions
        .declare_prop(
            vec![Type::U8],
            vec![
                PropVariant::indexed(unit(), |_| vec![Term::U8(2)]),
                PropVariant::indexed(unit(), |_| vec![Term::U8(3)]),
            ],
        )
        .unwrap()
}

fn or_left(prelude: &Prelude, p: &Term, q: &Term, proof: Proof) -> Proof {
    Proof::Construct {
        prop: prelude.or,
        variant: 0,
        params: vec![p.clone(), q.clone()],
        payload: vec![Term::proof(proof)],
    }
}

fn or_right(prelude: &Prelude, p: &Term, q: &Term, proof: Proof) -> Proof {
    Proof::Construct {
        prop: prelude.or,
        variant: 1,
        params: vec![p.clone(), q.clone()],
        payload: vec![Term::proof(proof)],
    }
}

fn truth(prelude: &Prelude) -> Proof {
    Proof::Construct {
        prop: prelude.truth,
        variant: 0,
        params: vec![],
        payload: vec![],
    }
}

// --- The stated gate conditions ---------------------------------------------

#[test]
fn an_indexed_proposition_match_supplies_its_index_equation() {
    // h: @Small(n)  |-  n == 2 || n == 3
    let (mut definitions, prelude) = Definitions::with_prelude();
    let small = declare_small(&mut definitions);
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    let n = Term::var(ctx.declare(Type::U8).unwrap());
    let h = ctx.assume(Term::PropApp(small, vec![n.clone()])).unwrap();

    let is_two = u8_eq(n.clone(), Term::U8(2));
    let is_three = u8_eq(n.clone(), Term::U8(3));
    let goal = prelude.or_prop(is_two.clone(), is_three.clone());
    let proof = Proof::CaseProof {
        scrutinee: Box::new(Proof::hyp(h)),
        goal: goal.clone(),
        arms: vec![
            // In each arm the single hypothesis is the index equation.
            Proof::arm(0, 1, |_, eqs| {
                or_left(&prelude, &is_two, &is_three, eqs[0].clone())
            }),
            Proof::arm(0, 1, |_, eqs| {
                or_right(&prelude, &is_two, &is_three, eqs[0].clone())
            }),
        ],
    };
    assert_eq!(check_proof(&mut ctx, &proof, &goal), Ok(()));

    // The equations really are per arm: swapping the arms fails.
    let swapped = Proof::CaseProof {
        scrutinee: Box::new(Proof::hyp(h)),
        goal: goal.clone(),
        arms: vec![
            Proof::arm(0, 1, |_, eqs| {
                or_right(&prelude, &is_two, &is_three, eqs[0].clone())
            }),
            Proof::arm(0, 1, |_, eqs| {
                or_left(&prelude, &is_two, &is_three, eqs[0].clone())
            }),
        ],
    };
    assert!(matches!(
        infer_proof(&mut ctx, &swapped),
        Err(KernelError::ProofMismatch { .. })
    ));

    // Constructing: Small::Three proves Small(3) and nothing else.
    let three = Proof::Construct {
        prop: small,
        variant: 1,
        params: vec![],
        payload: vec![],
    };
    assert_eq!(
        infer_proof(&mut ctx, &three),
        Ok(Term::PropApp(small, vec![Term::U8(3)]))
    );
    assert!(check_proof(&mut ctx, &three, &Term::PropApp(small, vec![Term::U8(5)])).is_err());
}

#[test]
fn a_match_on_a_proof_can_only_produce_a_proof() {
    let (mut definitions, prelude) = Definitions::with_prelude();
    let light = declare_light(&mut definitions);
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    let p = Term::var(ctx.declare(Type::Prop).unwrap());
    let q = Term::var(ctx.declare(Type::Prop).unwrap());
    let h = ctx.assume(prelude.or_prop(p.clone(), q.clone())).unwrap();

    // There is no way to say "match h { Left => 0, Right => 1 }": the only
    // term-level case needs data to scrutinize, and a proof is not data.
    let scrutinee = Term::proof(Proof::hyp(h));
    let choose = Term::case(
        scrutinee,
        Type::U8,
        vec![
            (1, Box::new(|_, _| Term::U8(0))),
            (1, Box::new(|_, _| Term::U8(1))),
        ],
    );
    assert!(matches!(
        infer_term(&mut ctx, &choose, Mode::Logical),
        Err(KernelError::NotCaseable(_))
    ));
    // And a term-level case cannot smuggle a proof result either.
    let l = Term::var(ctx.declare(Type::Enum(light)).unwrap());
    let trivial = Type::proof(prelude.truth_prop());
    let proof_valued = Term::case(
        l,
        trivial.clone(),
        vec![
            (0, Box::new(|_, _| Term::proof(truth(&prelude)))),
            (0, Box::new(|_, _| Term::proof(truth(&prelude)))),
        ],
    );
    assert_eq!(
        infer_term(&mut ctx, &proof_valued, Mode::Logical),
        Err(KernelError::ProofResult(trivial))
    );

    // What a match on a proof can do: prove q || p from p || q.
    let goal = prelude.or_prop(q.clone(), p.clone());
    let swap = Proof::CaseProof {
        scrutinee: Box::new(Proof::hyp(h)),
        goal: goal.clone(),
        arms: vec![
            Proof::arm(1, 0, |payload, _| {
                or_right(&prelude, &q, &p, Proof::OfTerm(payload[0].clone()))
            }),
            Proof::arm(1, 0, |payload, _| {
                or_left(&prelude, &q, &p, Proof::OfTerm(payload[0].clone()))
            }),
        ],
    };
    assert_eq!(check_proof(&mut ctx, &swap, &goal), Ok(()));
}

#[test]
fn a_variant_always_concludes_its_own_proposition() {
    // A conclusion is a list of arguments for the proposition being
    // declared, so "concluding False" cannot even be written. What can go
    // wrong is the arguments, and that is rejected.
    let (mut definitions, _) = Definitions::with_prelude();
    let wrong_arity = definitions.declare_prop(
        vec![Type::U8],
        vec![PropVariant::indexed(unit(), |_| vec![])],
    );
    assert_eq!(
        wrong_arity,
        Err(KernelError::FieldCount {
            expected: 1,
            found: 0
        })
    );
    let wrong_type = definitions.declare_prop(
        vec![Type::U8],
        vec![PropVariant::indexed(unit(), |_| vec![Term::Bool(true)])],
    );
    assert!(matches!(wrong_type, Err(KernelError::TypeMismatch { .. })));
    // A parameter cannot be a proof, so index equations stay independent.
    let proof_param =
        definitions.declare_prop(vec![Type::proof(u8_eq(Term::U8(1), Term::U8(1)))], vec![]);
    assert!(matches!(proof_param, Err(KernelError::ProofParameter(_))));
    // A payload cannot mention the proposition being declared: its identity
    // does not exist yet. The next identity to be issued is this one.
    let next = declare_small(&mut Definitions::with_prelude().0);
    let recursive = definitions.declare_prop(
        vec![],
        vec![PropVariant::Params(Type::Tuple(vec![Type::proof(
            Term::PropApp(next, vec![]),
        )]))],
    );
    assert_eq!(recursive, Err(KernelError::UnknownProp));
}

#[test]
fn constructor_disjointness_and_injectivity_are_derived() {
    let (mut definitions, prelude) = Definitions::with_prelude();
    let light = declare_light(&mut definitions);
    // enum Reading { Missing, Byte(u8) }
    let reading = definitions
        .declare_enum(&[unit(), Type::Tuple(vec![Type::U8])])
        .unwrap();
    let mut ctx = Context::with_definitions(Rc::new(definitions));

    // Red == Green => False. Send Red to True and Green to False with a case,
    // then carry a proof of True across the equation.
    let is_red = |value: Term| {
        Term::case(
            value,
            Type::Prop,
            vec![
                (0, Box::new(|_, _| prelude.truth_prop())),
                (0, Box::new(|_, _| prelude.falsehood_prop())),
            ],
        )
    };
    let claim = Term::eq(Type::Enum(light), red(light), green(light));
    let disjoint = Proof::implies_intro(claim.clone(), |h| {
        let red_case = is_red(red(light));
        let cases_equal = Proof::Transport {
            eq: Box::new(h),
            template: Term::eq(Type::Prop, red_case.clone(), is_red(Term::Bound(0))),
            proof: Box::new(Proof::Refl(red_case.clone())),
        };
        let true_is_red_case = Proof::transport(
            Proof::CaseStep(red_case.clone()),
            |hole| Term::eq(Type::Prop, hole, red_case.clone()),
            Proof::Refl(red_case.clone()),
        );
        let green_case_is_false = Proof::CaseStep(is_red(green(light)));
        // True == case Red == case Green == False
        let true_is_false = Proof::Transport {
            eq: Box::new(green_case_is_false),
            template: Term::eq(Type::Prop, prelude.truth_prop(), Term::Bound(0)),
            proof: Box::new(Proof::Transport {
                eq: Box::new(cases_equal),
                template: Term::eq(Type::Prop, prelude.truth_prop(), Term::Bound(0)),
                proof: Box::new(true_is_red_case),
            }),
        };
        Proof::transport(true_is_false, |hole| hole, truth(&prelude))
    });
    assert_eq!(
        check_proof(&mut ctx, &disjoint, &prelude.not_prop(claim)),
        Ok(())
    );

    // Byte(a) == Byte(b) => a == b, by projecting the payload with a case.
    let a = Term::var(ctx.declare(Type::U8).unwrap());
    let b = Term::var(ctx.declare(Type::U8).unwrap());
    let byte = |value: &Term| Term::Variant(reading, 1, vec![value.clone()]);
    let payload_of = |value: Term| {
        Term::case(
            value,
            Type::U8,
            vec![
                (0, Box::new(|_, _| Term::U8(0))),
                (1, Box::new(|payload, _| payload[0].clone())),
            ],
        )
    };
    let bytes_equal = ctx
        .assume(Term::eq(Type::Enum(reading), byte(&a), byte(&b)))
        .unwrap();
    let left = payload_of(byte(&a));
    let payloads_equal = Proof::Transport {
        eq: Box::new(Proof::hyp(bytes_equal)),
        template: u8_eq(left.clone(), payload_of(Term::Bound(0))),
        proof: Box::new(Proof::Refl(left.clone())),
    };
    let a_step = Proof::CaseStep(left);
    let b_step = Proof::CaseStep(payload_of(byte(&b)));
    let a_back = symm(&mut ctx, &a_step).unwrap();
    let chain = trans(&mut ctx, &a_back, &payloads_equal).unwrap();
    let chain = trans(&mut ctx, &chain, &b_step).unwrap();
    assert_eq!(check_proof(&mut ctx, &chain, &u8_eq(a, b)), Ok(()));
}

// --- Case analysis on data -----------------------------------------------------

#[test]
fn each_arm_of_a_case_on_data_learns_which_constructor_it_has() {
    // forall l: Light, l == Red || l == Green
    let (mut definitions, prelude) = Definitions::with_prelude();
    let light = declare_light(&mut definitions);
    let mut ctx = Context::with_definitions(Rc::new(definitions));

    let statement = Term::forall(Type::Enum(light), |l| {
        prelude.or_prop(
            Term::eq(Type::Enum(light), l.clone(), red(light)),
            Term::eq(Type::Enum(light), l, green(light)),
        )
    });
    let proof = Proof::forall_intro(Type::Enum(light), |l| {
        let is_red = Term::eq(Type::Enum(light), l.clone(), red(light));
        let is_green = Term::eq(Type::Enum(light), l.clone(), green(light));
        Proof::CaseData {
            scrutinee: l,
            goal: prelude.or_prop(is_red.clone(), is_green.clone()),
            arms: vec![
                Proof::arm(0, 1, |_, facts| {
                    or_left(&prelude, &is_red, &is_green, facts[0].clone())
                }),
                Proof::arm(0, 1, |_, facts| {
                    or_right(&prelude, &is_red, &is_green, facts[0].clone())
                }),
            ],
        }
    });
    assert_eq!(check_proof(&mut ctx, &proof, &statement), Ok(()));
}

#[test]
fn if_is_case_on_bool_and_each_branch_learns_the_condition() {
    let (definitions, prelude) = Definitions::with_prelude();
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    let b = Term::var(ctx.declare(Type::Bool).unwrap());

    // An executable if: arms are (false, true).
    let choose = Term::case(
        b.clone(),
        Type::U8,
        vec![
            (0, Box::new(|_, _| Term::U8(10))),
            (0, Box::new(|_, _| Term::U8(20))),
        ],
    );
    assert_eq!(
        infer_term(&mut ctx, &choose, Mode::Executable),
        Ok(Type::U8)
    );
    let when_true = Term::case(
        Term::Bool(true),
        Type::U8,
        vec![
            (0, Box::new(|_, _| Term::U8(10))),
            (0, Box::new(|_, _| Term::U8(20))),
        ],
    );
    assert_eq!(
        infer_proof(&mut ctx, &Proof::CaseStep(when_true.clone())),
        Ok(u8_eq(when_true, Term::U8(20)))
    );
    // A case on an unknown scrutinee does not reduce.
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::CaseStep(choose)),
        Err(KernelError::NoComputationStep(_))
    ));

    // Branch evidence: b == false || b == true.
    let is_false = Term::eq(Type::Bool, b.clone(), Term::Bool(false));
    let is_true = Term::eq(Type::Bool, b.clone(), Term::Bool(true));
    let goal = prelude.or_prop(is_false.clone(), is_true.clone());
    let proof = Proof::CaseData {
        scrutinee: b,
        goal: goal.clone(),
        arms: vec![
            Proof::arm(0, 1, |_, facts| {
                or_left(&prelude, &is_false, &is_true, facts[0].clone())
            }),
            Proof::arm(0, 1, |_, facts| {
                or_right(&prelude, &is_false, &is_true, facts[0].clone())
            }),
        ],
    };
    assert_eq!(check_proof(&mut ctx, &proof, &goal), Ok(()));
}

#[test]
fn a_payload_with_a_proof_field_gives_each_arm_its_evidence() {
    // enum Checked { None, Some(value: u8, @[value == 7]) }
    let (mut definitions, prelude) = Definitions::with_prelude();
    let some_payload = Type::tuple(|earlier| match earlier {
        [] => Some(Type::U8),
        [value] => Some(Type::proof(u8_eq(value.clone(), Term::U8(7)))),
        _ => None,
    });
    let checked = definitions.declare_enum(&[unit(), some_payload]).unwrap();
    let mut ctx = Context::with_definitions(Rc::new(definitions));

    let good = Term::Variant(
        checked,
        1,
        vec![Term::U8(7), Term::proof(Proof::Refl(Term::U8(7)))],
    );
    assert_eq!(
        infer_term(&mut ctx, &good, Mode::Executable),
        Ok(Type::Enum(checked))
    );
    let bad = Term::Variant(
        checked,
        1,
        vec![Term::U8(8), Term::proof(Proof::Refl(Term::U8(8)))],
    );
    assert!(matches!(
        infer_term(&mut ctx, &bad, Mode::Executable),
        Err(KernelError::ProofMismatch { .. })
    ));

    // In an executable case the data payload is executable and the proof
    // payload is not.
    let c = Term::var(ctx.declare(Type::Enum(checked)).unwrap());
    let take = Term::case(
        c.clone(),
        Type::U8,
        vec![
            (0, Box::new(|_, _| Term::U8(0))),
            (2, Box::new(|payload, _| payload[0].clone())),
        ],
    );
    assert_eq!(infer_term(&mut ctx, &take, Mode::Executable), Ok(Type::U8));

    // The arm's evidence is about the arm's own payload variable.
    let goal = prelude.truth_prop();
    let uses_evidence = Proof::CaseData {
        scrutinee: c,
        goal: goal.clone(),
        arms: vec![
            Proof::arm(0, 1, |_, _| truth(&prelude)),
            Proof::arm(2, 1, |payload, _| {
                // Feeding the evidence to something that demands a proof of
                // "value == 7" makes the kernel check what it proves.
                let needs_seven =
                    Proof::implies_intro(u8_eq(payload[0].clone(), Term::U8(7)), |_| {
                        truth(&prelude)
                    });
                Proof::implies_elim(needs_seven, Proof::OfTerm(payload[1].clone()))
            }),
        ],
    };
    assert_eq!(check_proof(&mut ctx, &uses_evidence, &goal), Ok(()));

    // The arm's fact is itself a well-formed proposition: its proof field is
    // in the canonical form proof(of_term(h)), not the bare variable.
    let c = Term::var(ctx.declare(Type::Enum(checked)).unwrap());
    let fact_is_well_formed = Proof::CaseData {
        scrutinee: c.clone(),
        goal: goal.clone(),
        arms: vec![
            Proof::arm(0, 1, |_, _| truth(&prelude)),
            Proof::arm(2, 1, |payload, facts| {
                let stated = Term::eq(
                    Type::Enum(checked),
                    c.clone(),
                    Term::Variant(
                        checked,
                        1,
                        vec![
                            payload[0].clone(),
                            Term::proof(Proof::OfTerm(payload[1].clone())),
                        ],
                    ),
                );
                Proof::implies_elim(
                    Proof::implies_intro(stated, |_| truth(&prelude)),
                    facts[0].clone(),
                )
            }),
        ],
    };
    assert_eq!(check_proof(&mut ctx, &fact_is_well_formed, &goal), Ok(()));
}

#[test]
fn arms_must_match_the_declaration() {
    let (mut definitions, prelude) = Definitions::with_prelude();
    let light = declare_light(&mut definitions);
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    let l = Term::var(ctx.declare(Type::Enum(light)).unwrap());

    let one_arm = Term::case(l.clone(), Type::U8, vec![(0, Box::new(|_, _| Term::U8(0)))]);
    assert_eq!(
        infer_term(&mut ctx, &one_arm, Mode::Logical),
        Err(KernelError::ArmCount {
            expected: 2,
            found: 1
        })
    );
    let extra_binder = Term::case(
        l.clone(),
        Type::U8,
        vec![
            (1, Box::new(|_, _| Term::U8(0))),
            (0, Box::new(|_, _| Term::U8(0))),
        ],
    );
    assert!(matches!(
        infer_term(&mut ctx, &extra_binder, Mode::Logical),
        Err(KernelError::ArmBinders { .. })
    ));
    let missing_fact = Proof::CaseData {
        scrutinee: l,
        goal: prelude.truth_prop(),
        arms: vec![
            Proof::arm(0, 0, |_, _| truth(&prelude)),
            Proof::arm(0, 1, |_, _| truth(&prelude)),
        ],
    };
    assert!(matches!(
        infer_proof(&mut ctx, &missing_fact),
        Err(KernelError::ArmBinders { .. })
    ));
    assert!(matches!(
        infer_term(&mut ctx, &Term::Variant(light, 2, vec![]), Mode::Logical),
        Err(KernelError::NoSuchVariant { .. })
    ));
}

// --- False, And, Exists, excluded middle --------------------------------------

#[test]
fn false_has_no_proofs_and_proves_anything() {
    let (definitions, prelude) = Definitions::with_prelude();
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    let impossible = ctx.assume(prelude.falsehood_prop()).unwrap();

    // A match with no arms proves any goal ...
    let anything = u8_eq(Term::U8(1), Term::U8(2));
    let ex_falso = Proof::CaseProof {
        scrutinee: Box::new(Proof::hyp(impossible)),
        goal: anything.clone(),
        arms: vec![],
    };
    assert_eq!(check_proof(&mut ctx, &ex_falso, &anything), Ok(()));
    // ... and yields a value of any type, even an executable one.
    let unreachable = Term::Absurd(Box::new(Proof::hyp(impossible)), Type::U8);
    assert_eq!(
        infer_term(&mut ctx, &unreachable, Mode::Executable),
        Ok(Type::U8)
    );
    // Absurdity needs an empty proposition.
    let not_empty = Term::Absurd(Box::new(truth(&prelude)), Type::U8);
    assert!(matches!(
        infer_term(&mut ctx, &not_empty, Mode::Logical),
        Err(KernelError::NotEmpty(_))
    ));
    // False has no constructor.
    let forged = Proof::Construct {
        prop: prelude.falsehood,
        variant: 0,
        params: vec![],
        payload: vec![],
    };
    assert!(matches!(
        infer_proof(&mut ctx, &forged),
        Err(KernelError::NoSuchVariant { .. })
    ));
}

#[test]
fn conjunction_is_a_single_variant_proposition() {
    let (definitions, prelude) = Definitions::with_prelude();
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    let p = Term::var(ctx.declare(Type::Prop).unwrap());
    let q = Term::var(ctx.declare(Type::Prop).unwrap());
    let hp = ctx.assume(p.clone()).unwrap();
    let hq = ctx.assume(q.clone()).unwrap();

    let both = Proof::Construct {
        prop: prelude.and,
        variant: 0,
        params: vec![p.clone(), q.clone()],
        payload: vec![Term::proof(Proof::hyp(hp)), Term::proof(Proof::hyp(hq))],
    };
    assert_eq!(
        check_proof(&mut ctx, &both, &prelude.and_prop(p.clone(), q.clone())),
        Ok(())
    );
    // The payload is checked against the parameters it was given.
    let crossed = Proof::Construct {
        prop: prelude.and,
        variant: 0,
        params: vec![p.clone(), q.clone()],
        payload: vec![Term::proof(Proof::hyp(hq)), Term::proof(Proof::hyp(hp))],
    };
    assert!(infer_proof(&mut ctx, &crossed).is_err());

    let right = Proof::CaseProof {
        scrutinee: Box::new(both),
        goal: q.clone(),
        arms: vec![Proof::arm(2, 0, |payload, _| {
            Proof::OfTerm(payload[1].clone())
        })],
    };
    assert_eq!(check_proof(&mut ctx, &right, &q), Ok(()));
}

#[test]
fn an_existential_is_opened_only_to_prove_something_else() {
    let (definitions, _) = Definitions::with_prelude();
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    let n = Term::var(ctx.declare(Type::U8).unwrap());

    // exists k { n == k.wrapping_add(k) }, witnessed by a ghost value.
    let doubled = |k: Term| {
        u8_eq(
            n.clone(),
            Term::op(Op::WrappingAdd, MachineInt::U8, vec![k.clone(), k]),
        )
    };
    let claim = Term::exists(Type::U8, doubled);
    let k = Term::var(ctx.declare_ghost(Type::U8).unwrap());
    let fact = ctx.assume(doubled(k.clone())).unwrap();
    let intro = Proof::ExistsIntro {
        prop: claim.clone(),
        witness: k,
        proof: Box::new(Proof::hyp(fact)),
    };
    assert_eq!(check_proof(&mut ctx, &intro, &claim), Ok(()));
    let wrong_witness = Proof::ExistsIntro {
        prop: claim.clone(),
        witness: Term::U8(0),
        proof: Box::new(Proof::hyp(fact)),
    };
    assert!(infer_proof(&mut ctx, &wrong_witness).is_err());

    // Opening it: exists k { k + k == n }, the same fact flipped.
    let flipped = Term::exists(Type::U8, |k| {
        u8_eq(
            Term::op(Op::WrappingAdd, MachineInt::U8, vec![k.clone(), k]),
            n.clone(),
        )
    });
    let h = ctx.assume(claim).unwrap();
    // From n == w + w, get w + w == n, and repackage it.
    let reopen = Proof::ExistsElim {
        exists: Box::new(Proof::hyp(h)),
        goal: flipped.clone(),
        arm: Proof::arm(1, 1, |witness, facts| {
            let w = witness[0].clone();
            let target = n.clone();
            Proof::ExistsIntro {
                prop: flipped.clone(),
                witness: w,
                proof: Box::new(Proof::transport(
                    facts[0].clone(),
                    |hole| u8_eq(hole, target.clone()),
                    Proof::Refl(target.clone()),
                )),
            }
        }),
    };
    assert_eq!(check_proof(&mut ctx, &reopen, &flipped), Ok(()));

    // The witness cannot escape into the goal.
    let mut leaked = None;
    let leak = Proof::ExistsElim {
        exists: Box::new(Proof::hyp(h)),
        goal: flipped,
        arm: Proof::arm(1, 1, |witness, facts| {
            leaked = Some(witness[0].clone());
            facts[0].clone()
        }),
    };
    let _ = infer_proof(&mut ctx, &leak);
    let goal_about_witness = u8_eq(leaked.unwrap(), Term::U8(0));
    assert!(matches!(
        infer_term(&mut ctx, &goal_about_witness, Mode::Logical),
        Err(KernelError::UnknownVariable(_))
    ));
}

#[test]
fn excluded_middle_is_available_and_its_use_is_recorded() {
    let (mut definitions, prelude) = Definitions::with_prelude();
    let signature = Type::function(1, |params| match params {
        [] => Type::Prop,
        [p] => Type::proof(prelude.or_prop(p.clone(), prelude.not_prop(p.clone()))),
        _ => unreachable!(),
    });
    // math fn decide(p: Prop) -> @[p || !p] { excluded_middle(p) }
    let decide = definitions
        .declare_fn(&signature, |params| {
            Term::proof(Proof::ExcludedMiddle(params[0].clone()))
        })
        .unwrap();
    // A lemma that calls it is classical too; one that does not is not.
    let via = definitions
        .declare_fn(&signature, |params| {
            Term::proof(Proof::OfTerm(Term::call(
                Term::Fn(decide),
                vec![params[0].clone()],
            )))
        })
        .unwrap();
    let trivial = Type::function(0, |_| Type::proof(prelude.truth_prop()));
    let constructive = definitions
        .declare_fn(&trivial, |_| Term::proof(truth(&prelude)))
        .unwrap();
    assert!(definitions.is_classical(decide));
    assert!(definitions.is_classical(via));
    assert!(!definitions.is_classical(constructive));

    let shared = Rc::new(definitions);
    let mut ctx = Context::with_definitions(Rc::clone(&shared));
    let p = Term::var(ctx.declare(Type::Prop).unwrap());
    let em = Proof::ExcludedMiddle(p.clone());
    assert_eq!(
        check_proof(
            &mut ctx,
            &em,
            &prelude.or_prop(p.clone(), prelude.not_prop(p.clone()))
        ),
        Ok(())
    );
    assert!(proof_is_classical(&shared, &em));
    assert!(proof_is_classical(
        &shared,
        &Proof::OfTerm(Term::call(Term::Fn(via), vec![p.clone()]))
    ));
    assert!(!proof_is_classical(&shared, &truth(&prelude)));

    // Double negation elimination, the classical principle programmers
    // expect: from !!p, by cases on p || !p.
    let not_p = prelude.not_prop(p.clone());
    let not_not_p = prelude.not_prop(not_p.clone());
    let h = ctx.assume(not_not_p).unwrap();
    let dne = Proof::CaseProof {
        scrutinee: Box::new(Proof::ExcludedMiddle(p.clone())),
        goal: p.clone(),
        arms: vec![
            Proof::arm(1, 0, |payload, _| Proof::OfTerm(payload[0].clone())),
            Proof::arm(1, 0, |payload, _| Proof::CaseProof {
                scrutinee: Box::new(Proof::implies_elim(
                    Proof::hyp(h),
                    Proof::OfTerm(payload[0].clone()),
                )),
                goal: p.clone(),
                arms: vec![],
            }),
        ],
    };
    assert_eq!(check_proof(&mut ctx, &dne, &p), Ok(()));
    assert!(proof_is_classical(&shared, &dne));

    // Without the prelude there is nothing to state it with.
    let mut bare = Context::new();
    let q = Term::var(bare.declare(Type::Prop).unwrap());
    assert_eq!(
        infer_proof(&mut bare, &Proof::ExcludedMiddle(q)),
        Err(KernelError::NoPrelude)
    );
}

#[test]
fn comparison_covers_the_new_terms() {
    let (mut definitions, prelude) = Definitions::with_prelude();
    let light = declare_light(&mut definitions);
    assert!(same(&red(light), &red(light)));
    assert!(!same(&red(light), &green(light)));
    assert!(same(
        &Term::exists(Type::U8, |x| u8_eq(x.clone(), x)),
        &Term::exists(Type::U8, |y| u8_eq(y.clone(), y)),
    ));
    assert!(!same(&prelude.truth_prop(), &prelude.falsehood_prop()));
}

#[test]
fn a_branch_of_a_math_function_knows_which_branch_it_is() {
    // math fn preserve(n: u8) -> (out: u8, @[out == n]) {
    //     if n == 0 { (0, _) } else { (n, _) }
    // }
    // The first hole needs the branch fact: the comparison was true, so
    // n == 0, so 0 == n.
    use locus::kernel::Axiom;
    use locus::kernel::derive::Chain;
    let (mut definitions, _) = Definitions::with_prelude();
    let result_for = |n: &Term| {
        let n = n.clone();
        Type::tuple(move |earlier| match earlier {
            [] => Some(Type::U8),
            [out] => Some(Type::proof(u8_eq(out.clone(), n.clone()))),
            _ => None,
        })
    };
    let signature = Type::function(1, |params| match params {
        [] => Type::U8,
        [n] => result_for(n),
        _ => unreachable!(),
    });
    let body = |n: &Term, use_the_fact: bool| {
        let n = n.clone();
        let comparison = Term::cmp(CmpOp::Eq, MachineInt::U8, n.clone(), Term::U8(0));
        let result = result_for(&n);
        let (result_true, result_false) = (result.clone(), result.clone());
        let (n_true, n_false) = (n.clone(), n.clone());
        let reflected = comparison.clone();
        Term::case(
            comparison,
            result,
            vec![
                // false: return n itself
                (
                    0,
                    Box::new(move |_, _| {
                        Term::tuple(
                            &result_false,
                            vec![n_false.clone(), Term::proof(Proof::Refl(n_false))],
                        )
                    }),
                ),
                // true: return 0, with 0 == n from the arm's fact
                (
                    0,
                    Box::new(move |_, fact| {
                        // The fact reflects to an equality of the views;
                        // wrap_view carries it to the bytes: n is
                        // wrap(view(n)), which is wrap(view(0)), which is 0.
                        let views_equal = Proof::implies_elim(
                            Proof::Axiom(Axiom::CmpReflect(reflected, true)),
                            fact,
                        );
                        let view = |x: Term| Term::view(MachineInt::U8, x);
                        let n_is_zero = Chain::new(Type::U8, n_true.clone())
                            .step_rev(
                                &Term::wrap(MachineInt::U8, view(n_true.clone())),
                                Proof::Axiom(Axiom::WrapView(MachineInt::U8, n_true.clone())),
                            )
                            .rewrite(|hole| Term::wrap(MachineInt::U8, hole), views_equal)
                            .step(Proof::Axiom(Axiom::WrapView(MachineInt::U8, Term::U8(0))))
                            .finish();
                        let zero_is_n = Proof::transport(
                            n_is_zero,
                            |hole| u8_eq(hole, n_true.clone()),
                            Proof::Refl(n_true.clone()),
                        );
                        let evidence = if use_the_fact {
                            zero_is_n
                        } else {
                            Proof::Refl(Term::U8(0))
                        };
                        Term::tuple(&result_true, vec![Term::U8(0), Term::proof(evidence)])
                    }),
                ),
            ],
        )
    };
    assert!(
        definitions
            .declare_fn(&signature, |params| body(&params[0], true))
            .is_ok()
    );
    // Without the fact the branch cannot justify returning 0.
    assert!(matches!(
        definitions.declare_fn(&signature, |params| body(&params[0], false)),
        Err(KernelError::ProofMismatch { .. })
    ));

    // The fact is discharged by reflexivity when the case reduces, and the
    // whole function evaluates.
    let preserve = definitions
        .declare_fn(&signature, |params| body(&params[0], true))
        .unwrap();
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    for byte in [0u8, 7] {
        let out = Term::proj(Term::call(Term::Fn(preserve), vec![Term::U8(byte)]), 0);
        assert_eq!(
            check_proof(
                &mut ctx,
                &Proof::Evaluate(out.clone()),
                &u8_eq(out, Term::U8(byte))
            ),
            Ok(())
        );
    }
    let reduced = Term::case(
        Term::Bool(true),
        Type::U8,
        vec![
            (0, Box::new(|_, _| Term::U8(1))),
            (0, Box::new(|_, _| Term::U8(2))),
        ],
    );
    assert_eq!(
        infer_proof(&mut ctx, &Proof::CaseStep(reduced.clone())),
        Ok(u8_eq(reduced, Term::U8(2)))
    );
}
