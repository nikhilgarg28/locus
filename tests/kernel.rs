//! Acceptance tests for kernel gate K1 (docs/core-plan.md): terms, the
//! three-kinded context with upgrade, comparison up to renaming, equality
//! with reflexivity and transport, and internal `Forall` and `Implies`.
//! Every term here is written by hand; nothing comes from the parser.

use locus::kernel::{
    Context, HypId, KernelError, Mode, Proof, Term, Type, VarId, check_proof, infer_proof,
    infer_term, same,
};

fn u8_eq(left: Term, right: Term) -> Term {
    Term::eq(Type::U8, left, right)
}

fn add_one(term: Term) -> Term {
    Term::wrapping_add(term, Term::U8(1))
}

/// From `eq: a == b`, a proof of `b == a`.
fn symmetry(eq: Proof, a: Term) -> Proof {
    Proof::transport(eq, |hole| u8_eq(hole, a.clone()), Proof::Refl(a.clone()))
}

// --- The four stated gate conditions ---------------------------------------

#[test]
fn a_true_equality_checks() {
    let mut ctx = Context::new();
    let goal = u8_eq(Term::U8(3), Term::U8(3));
    assert_eq!(
        check_proof(&mut ctx, &Proof::Refl(Term::U8(3)), &goal),
        Ok(())
    );
}

#[test]
fn a_false_equality_is_rejected() {
    let mut ctx = Context::new();
    let goal = u8_eq(Term::U8(3), Term::U8(4));
    for candidate in [Term::U8(3), Term::U8(4)] {
        let result = check_proof(&mut ctx, &Proof::Refl(candidate), &goal);
        assert!(matches!(result, Err(KernelError::ProofMismatch { .. })));
    }
}

#[test]
fn reflexivity_does_not_compute() {
    // The kernel has no conversion: 1 + 1 == 2 needs a computation axiom (K2).
    let mut ctx = Context::new();
    let goal = u8_eq(add_one(Term::U8(1)), Term::U8(2));
    let result = check_proof(&mut ctx, &Proof::Refl(Term::U8(2)), &goal);
    assert!(matches!(result, Err(KernelError::ProofMismatch { .. })));
}

#[test]
fn a_fact_in_scope_discharges_an_identical_goal_only() {
    let mut ctx = Context::new();
    let n = Term::var(ctx.declare(Type::U8));
    let fact = ctx.assume(u8_eq(n.clone(), Term::U8(3))).unwrap();

    let same_goal = u8_eq(n.clone(), Term::U8(3));
    assert_eq!(check_proof(&mut ctx, &Proof::hyp(fact), &same_goal), Ok(()));

    let other_goal = u8_eq(n.clone(), Term::U8(4));
    assert!(check_proof(&mut ctx, &Proof::hyp(fact), &other_goal).is_err());

    // Not even the symmetric statement is identical.
    let flipped = u8_eq(Term::U8(3), n);
    assert!(check_proof(&mut ctx, &Proof::hyp(fact), &flipped).is_err());
}

#[test]
fn a_ghost_variable_is_rejected_in_an_executable_term() {
    let mut ctx = Context::new();
    let n = ctx.declare(Type::U8);
    let k = ctx.declare_ghost(Type::U8);
    let sum = Term::wrapping_add(Term::var(n), Term::var(k));

    assert_eq!(
        infer_term(&mut ctx, &sum, Mode::Executable),
        Err(KernelError::GhostInExecutable(k))
    );
    // The upgraded context makes the same term an ordinary logical term.
    assert_eq!(infer_term(&mut ctx, &sum, Mode::Logical), Ok(Type::U8));
    // An executable term may use executable variables.
    assert_eq!(
        infer_term(&mut ctx, &add_one(Term::var(n)), Mode::Executable),
        Ok(Type::U8)
    );
}

#[test]
fn ghost_and_executable_variables_mix_freely_inside_a_proposition() {
    let mut ctx = Context::new();
    let n = Term::var(ctx.declare(Type::U8));
    let k = Term::var(ctx.declare_ghost(Type::U8));
    let claim = u8_eq(n, Term::wrapping_add(k.clone(), k));
    assert_eq!(infer_term(&mut ctx, &claim, Mode::Logical), Ok(Type::Prop));
    assert!(ctx.assume(claim).is_ok());
}

#[test]
fn propositions_are_ghost_by_type() {
    let mut ctx = Context::new();
    let p = ctx.declare(Type::Prop);
    assert_eq!(
        infer_term(&mut ctx, &Term::var(p), Mode::Executable),
        Err(KernelError::GhostInExecutable(p))
    );
    let claim = u8_eq(Term::U8(1), Term::U8(1));
    assert_eq!(
        infer_term(&mut ctx, &claim, Mode::Executable),
        Err(KernelError::GhostTypeInExecutable(Type::Prop))
    );
}

// --- Transport ---------------------------------------------------------------

#[test]
fn symmetry_by_transport() {
    let mut ctx = Context::new();
    let a = Term::var(ctx.declare(Type::U8));
    let b = Term::var(ctx.declare(Type::U8));
    let h = ctx.assume(u8_eq(a.clone(), b.clone())).unwrap();

    let proof = symmetry(Proof::hyp(h), a.clone());
    assert_eq!(check_proof(&mut ctx, &proof, &u8_eq(b, a)), Ok(()));
}

#[test]
fn transitivity_by_transport() {
    let mut ctx = Context::new();
    let a = Term::var(ctx.declare(Type::U8));
    let b = Term::var(ctx.declare(Type::U8));
    let c = Term::var(ctx.declare(Type::U8));
    let ab = ctx.assume(u8_eq(a.clone(), b.clone())).unwrap();
    let bc = ctx.assume(u8_eq(b, c.clone())).unwrap();

    // Rewrite the right side of `a == b` along `b == c`.
    let left = a.clone();
    let proof = Proof::transport(
        Proof::hyp(bc),
        |hole| u8_eq(left.clone(), hole),
        Proof::hyp(ab),
    );
    assert_eq!(check_proof(&mut ctx, &proof, &u8_eq(a, c)), Ok(()));
}

#[test]
fn congruence_by_transport() {
    let mut ctx = Context::new();
    let a = Term::var(ctx.declare(Type::U8));
    let b = Term::var(ctx.declare(Type::U8));
    let h = ctx.assume(u8_eq(a.clone(), b.clone())).unwrap();

    let left = add_one(a.clone());
    let proof = Proof::transport(
        Proof::hyp(h),
        |hole| u8_eq(left.clone(), add_one(hole)),
        Proof::Refl(add_one(a.clone())),
    );
    assert_eq!(
        check_proof(&mut ctx, &proof, &u8_eq(add_one(a), add_one(b))),
        Ok(())
    );
}

#[test]
fn transport_rewrites_every_occurrence_the_template_names_and_no_other() {
    let mut ctx = Context::new();
    let a = Term::var(ctx.declare(Type::U8));
    let b = Term::var(ctx.declare(Type::U8));
    let h = ctx.assume(u8_eq(a.clone(), b.clone())).unwrap();
    let aa = ctx.assume(u8_eq(a.clone(), a.clone())).unwrap();

    // The template names only the second occurrence.
    let left = a.clone();
    let proof = Proof::transport(
        Proof::hyp(h),
        |hole| u8_eq(left.clone(), hole),
        Proof::hyp(aa),
    );
    assert_eq!(infer_proof(&mut ctx, &proof), Ok(u8_eq(a, b)));
}

#[test]
fn transport_checks_its_premise() {
    let mut ctx = Context::new();
    let a = Term::var(ctx.declare(Type::U8));
    let b = Term::var(ctx.declare(Type::U8));
    let h = ctx.assume(u8_eq(a.clone(), b.clone())).unwrap();

    // The premise must prove template[a]; Refl(b) proves template[b] instead.
    let target = b.clone();
    let proof = Proof::transport(
        Proof::hyp(h),
        |hole| u8_eq(hole, target.clone()),
        Proof::Refl(b),
    );
    assert!(matches!(
        infer_proof(&mut ctx, &proof),
        Err(KernelError::ProofMismatch { .. })
    ));
}

#[test]
fn transport_needs_an_equality_and_a_propositional_template() {
    let mut ctx = Context::new();
    let a = Term::var(ctx.declare(Type::U8));
    let claim = u8_eq(a.clone(), a.clone());
    let not_eq = ctx
        .assume(Term::implies(claim.clone(), claim.clone()))
        .unwrap();
    let eq = ctx.assume(claim).unwrap();

    let along_implication =
        Proof::transport(Proof::hyp(not_eq), |hole| hole, Proof::Refl(a.clone()));
    assert!(matches!(
        infer_proof(&mut ctx, &along_implication),
        Err(KernelError::NotAnEquality(_))
    ));

    // A template whose body is a u8, not a proposition.
    let data_template = Proof::transport(Proof::hyp(eq), add_one, Proof::Refl(a));
    assert_eq!(
        infer_proof(&mut ctx, &data_template),
        Err(KernelError::TypeMismatch {
            expected: Type::Prop,
            found: Type::U8
        })
    );
}

#[test]
fn equality_between_propositions_transports_a_proof() {
    // The shape of unfolding a predicate: from p == q and a proof of p, get q.
    let mut ctx = Context::new();
    let p = Term::var(ctx.declare(Type::Prop));
    let q = Term::var(ctx.declare(Type::Prop));
    let p_is_q = ctx
        .assume(Term::eq(Type::Prop, p.clone(), q.clone()))
        .unwrap();
    let p_holds = ctx.assume(p).unwrap();

    let proof = Proof::transport(Proof::hyp(p_is_q), |hole| hole, Proof::hyp(p_holds));
    assert_eq!(check_proof(&mut ctx, &proof, &q), Ok(()));
}

// --- Implies and Forall ------------------------------------------------------

#[test]
fn implication_introduction_and_elimination() {
    let mut ctx = Context::new();
    let n = Term::var(ctx.declare(Type::U8));
    let claim = u8_eq(n, Term::U8(3));

    let identity = Proof::implies_intro(claim.clone(), |h| h);
    let goal = Term::implies(claim.clone(), claim.clone());
    assert_eq!(check_proof(&mut ctx, &identity, &goal), Ok(()));

    let fact = ctx.assume(claim.clone()).unwrap();
    let applied = Proof::implies_elim(identity.clone(), Proof::hyp(fact));
    assert_eq!(check_proof(&mut ctx, &applied, &claim), Ok(()));

    // Modus ponens checks that the argument proves the premise.
    let wrong = Proof::implies_elim(identity, Proof::Refl(Term::U8(3)));
    assert!(matches!(
        infer_proof(&mut ctx, &wrong),
        Err(KernelError::ProofMismatch { .. })
    ));
    let not_a_function = Proof::implies_elim(Proof::hyp(fact), Proof::hyp(fact));
    assert!(matches!(
        infer_proof(&mut ctx, &not_a_function),
        Err(KernelError::NotAnImplication(_))
    ));
}

#[test]
fn a_hypothesis_is_scoped_to_its_introduction() {
    let mut ctx = Context::new();
    let claim = u8_eq(Term::U8(1), Term::U8(2));

    // Smuggle the bound hypothesis out of the closure.
    let mut escaped = None;
    let inside = Proof::implies_intro(claim.clone(), |h| {
        escaped = Some(h.clone());
        h
    });
    assert!(infer_proof(&mut ctx, &inside).is_ok());
    assert!(matches!(
        infer_proof(&mut ctx, &escaped.unwrap()),
        Err(KernelError::UnknownHypothesis(_))
    ));
    // An invented identity is no better.
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::hyp(HypId::fresh())),
        Err(KernelError::UnknownHypothesis(_))
    ));
    // And a false claim stays unproved: nothing above put it in the context.
    assert!(check_proof(&mut ctx, &Proof::Refl(Term::U8(1)), &claim).is_err());
}

#[test]
fn universal_introduction_and_elimination() {
    let mut ctx = Context::new();
    let all_self_equal = Term::forall(Type::U8, |x| u8_eq(x.clone(), x));
    let proof = Proof::forall_intro(Type::U8, Proof::Refl);
    assert_eq!(check_proof(&mut ctx, &proof, &all_self_equal), Ok(()));

    let n = Term::var(ctx.declare(Type::U8));
    let at_n = Proof::forall_elim(proof.clone(), add_one(n.clone()));
    assert_eq!(
        infer_proof(&mut ctx, &at_n),
        Ok(u8_eq(add_one(n.clone()), add_one(n)))
    );

    let ill_typed = Proof::forall_elim(proof, Term::Bool(true));
    assert_eq!(
        infer_proof(&mut ctx, &ill_typed),
        Err(KernelError::TypeMismatch {
            expected: Type::U8,
            found: Type::Bool
        })
    );
}

#[test]
fn generalization_cannot_capture_a_context_variable() {
    // From a fact about one particular n, "forall x, x == 3" must not follow.
    let mut ctx = Context::new();
    let n = Term::var(ctx.declare(Type::U8));
    let fact = ctx.assume(u8_eq(n, Term::U8(3))).unwrap();

    let bogus = Proof::forall_intro(Type::U8, |_| Proof::hyp(fact));
    let everything_is_three = Term::forall(Type::U8, |x| u8_eq(x, Term::U8(3)));
    assert!(check_proof(&mut ctx, &bogus, &everything_is_three).is_err());
}

#[test]
fn bound_variable_names_do_not_matter() {
    let first = Term::forall(Type::U8, |x| u8_eq(x.clone(), x));
    let second = Term::forall(Type::U8, |y| u8_eq(y.clone(), y));
    assert!(same(&first, &second));

    let nested = Term::forall(Type::U8, |x| {
        Term::forall(Type::U8, |y| u8_eq(x.clone(), y))
    });
    let swapped = Term::forall(Type::U8, |x| {
        Term::forall(Type::U8, |y| u8_eq(y, x.clone()))
    });
    assert!(!same(&nested, &swapped));
}

#[test]
fn a_program_path_statement() {
    // The shape of section 6.3: for all n, given the branch fact n == 3,
    // n.wrapping_add(1) == 3.wrapping_add(1).
    let mut ctx = Context::new();
    let statement = Term::forall(Type::U8, |n| {
        Term::implies(
            u8_eq(n.clone(), Term::U8(3)),
            u8_eq(add_one(n), add_one(Term::U8(3))),
        )
    });
    let proof = Proof::forall_intro(Type::U8, |n| {
        let branch_fact = u8_eq(n.clone(), Term::U8(3));
        Proof::implies_intro(branch_fact, |h| {
            let left = add_one(n.clone());
            Proof::transport(
                h,
                |hole| u8_eq(left.clone(), add_one(hole)),
                Proof::Refl(add_one(n.clone())),
            )
        })
    });
    assert_eq!(check_proof(&mut ctx, &proof, &statement), Ok(()));
    // Checking left nothing behind in the context.
    assert!(check_proof(&mut ctx, &proof, &statement).is_ok());
}

#[test]
fn nested_binders_in_proofs_line_up() {
    // forall a b, a == b => b == a, with the transport under two binders.
    let mut ctx = Context::new();
    let statement = Term::forall(Type::U8, |a| {
        Term::forall(Type::U8, |b| {
            Term::implies(u8_eq(a.clone(), b.clone()), u8_eq(b, a.clone()))
        })
    });
    let proof = Proof::forall_intro(Type::U8, |a| {
        Proof::forall_intro(Type::U8, |b| {
            Proof::implies_intro(u8_eq(a.clone(), b), |h| symmetry(h, a.clone()))
        })
    });
    assert_eq!(check_proof(&mut ctx, &proof, &statement), Ok(()));

    // Instantiate it at two context variables and apply it to a fact.
    let x = Term::var(ctx.declare(Type::U8));
    let y = Term::var(ctx.declare(Type::U8));
    let xy = ctx.assume(u8_eq(x.clone(), y.clone())).unwrap();
    let used = Proof::implies_elim(
        Proof::forall_elim(Proof::forall_elim(proof, x.clone()), y.clone()),
        Proof::hyp(xy),
    );
    assert_eq!(check_proof(&mut ctx, &used, &u8_eq(y, x)), Ok(()));
}

// --- Ill-formed input --------------------------------------------------------

#[test]
fn ill_formed_terms_are_rejected() {
    let mut ctx = Context::new();
    assert_eq!(
        infer_term(&mut ctx, &Term::Bound(0), Mode::Logical),
        Err(KernelError::DanglingBound)
    );
    let stranger = VarId::fresh();
    assert_eq!(
        infer_term(&mut ctx, &Term::var(stranger), Mode::Logical),
        Err(KernelError::UnknownVariable(stranger))
    );
    assert_eq!(
        infer_term(
            &mut ctx,
            &Term::eq(Type::U8, Term::U8(1), Term::Bool(true)),
            Mode::Logical
        ),
        Err(KernelError::TypeMismatch {
            expected: Type::U8,
            found: Type::Bool
        })
    );
    assert_eq!(
        infer_term(
            &mut ctx,
            &Term::Prim(locus::kernel::Prim::WrappingAdd, vec![Term::U8(1)]),
            Mode::Logical
        ),
        Err(KernelError::WrongArity {
            expected: 2,
            found: 1
        })
    );
    // Only a proposition can be assumed or be a goal.
    assert!(ctx.assume(Term::U8(1)).is_err());
    assert!(check_proof(&mut ctx, &Proof::Refl(Term::U8(1)), &Term::U8(1)).is_err());
}

#[test]
fn a_variable_leaves_scope_with_its_binder() {
    let mut ctx = Context::new();
    let mut escaped = None;
    let proof = Proof::forall_intro(Type::U8, |x| {
        escaped = Some(x.clone());
        Proof::Refl(x)
    });
    assert!(infer_proof(&mut ctx, &proof).is_ok());
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::Refl(escaped.unwrap())),
        Err(KernelError::UnknownVariable(_))
    ));
}
