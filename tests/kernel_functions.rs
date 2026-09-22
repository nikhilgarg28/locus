//! Acceptance tests for kernel gate K3 (the kernel contract in atlas.html): math functions
//! and their defining equations, function types and values, `Prop` as a
//! value, and the derived `rewrite`, `unfold`, and `fold` forms.
//! Every term here is written by hand; nothing comes from the parser.

use std::rc::Rc;

use locus::kernel::derive::{fold, rewrite, symm, trans, unfold};
use locus::kernel::{
    Context, Definitions, FnId, KernelError, MachineInt, Mode, Op, Proof, Term, Type, check_proof,
    infer_proof, infer_term, same,
};

fn u8_eq(left: Term, right: Term) -> Term {
    Term::eq(Type::U8, left, right)
}

fn add_one(term: Term) -> Term {
    Term::op(Op::WrappingAdd, MachineInt::U8, vec![term, Term::U8(1)])
}

fn u8_to_u8() -> Type {
    Type::function(1, |params| match params {
        [] | [_] => Type::U8,
        _ => unreachable!(),
    })
}

/// `math fn successor(n: u8) -> u8 { n.wrapping_add(1) }`
fn declare_successor(definitions: &mut Definitions) -> FnId {
    definitions
        .declare_fn(&u8_to_u8(), |params| add_one(params[0].clone()))
        .unwrap()
}

/// `math fn is_three(x: u8) -> Prop { [x == 3] }`
fn declare_is_three(definitions: &mut Definitions) -> FnId {
    let signature = Type::function(1, |params| match params {
        [] => Type::U8,
        _ => Type::Prop,
    });
    definitions
        .declare_fn(&signature, |params| u8_eq(params[0].clone(), Term::U8(3)))
        .unwrap()
}

fn call(id: FnId, arguments: Vec<Term>) -> Term {
    Term::call(Term::Fn(id), arguments)
}

// --- The stated gate conditions ---------------------------------------------

#[test]
fn an_explicit_unfolding_step_checks() {
    let mut definitions = Definitions::new();
    let is_three = declare_is_three(&mut definitions);
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    let n = Term::var(ctx.declare(Type::U8).unwrap());
    let claim = call(is_three, vec![n.clone()]);
    let h = ctx.assume(claim.clone()).unwrap();
    let unfolded_claim = u8_eq(n.clone(), Term::U8(3));

    // The kernel never unfolds on its own.
    assert!(matches!(
        check_proof(&mut ctx, &Proof::hyp(h), &unfolded_claim),
        Err(KernelError::ProofMismatch { .. })
    ));

    // The defining equation is an equality between propositions ...
    let equation = Proof::Definition(claim.clone());
    assert_eq!(
        infer_proof(&mut ctx, &equation),
        Ok(Term::eq(Type::Prop, claim.clone(), unfolded_claim.clone()))
    );
    // ... and unfolding is transport along it, written out by hand here.
    let by_hand = Proof::transport(equation, |hole| hole, Proof::hyp(h));
    assert_eq!(check_proof(&mut ctx, &by_hand, &unfolded_claim), Ok(()));

    // The derived forms build the same kind of proof.
    let unfolded = unfold(&mut ctx, is_three, &Proof::hyp(h)).unwrap();
    assert_eq!(check_proof(&mut ctx, &unfolded, &unfolded_claim), Ok(()));
    let refolded = fold(&mut ctx, is_three, &unfolded, &claim).unwrap();
    assert_eq!(check_proof(&mut ctx, &refolded, &claim), Ok(()));
}

#[test]
fn a_function_valued_definition_gives_equality_at_a_function_type() {
    // math fn select() -> math fn(u8) -> u8 { successor }
    let mut definitions = Definitions::new();
    let successor = declare_successor(&mut definitions);
    let returns_function = Type::function(0, |_| u8_to_u8());
    let select = definitions
        .declare_fn(&returns_function, |_| Term::Fn(successor))
        .unwrap();
    let mut ctx = Context::with_definitions(Rc::new(definitions));

    let select_call = call(select, vec![]);
    let equation = Proof::Definition(select_call.clone());
    let goal = Term::eq(u8_to_u8(), select_call.clone(), Term::Fn(successor));
    assert_eq!(check_proof(&mut ctx, &equation, &goal), Ok(()));

    // select()(3) == 4, one explicit step at a time.
    let applied = Term::call(select_call.clone(), vec![Term::U8(3)]);
    let start = Proof::Refl(applied.clone());
    let left = applied.clone();
    let to_successor = Proof::transport(
        equation,
        |hole| u8_eq(left.clone(), Term::call(hole, vec![Term::U8(3)])),
        start,
    );
    let unfold_successor = Proof::Definition(call(successor, vec![Term::U8(3)]));
    let evaluate = Proof::Literal(add_one(Term::U8(3)));
    let chain = trans(&mut ctx, &to_successor, &unfold_successor).unwrap();
    let chain = trans(&mut ctx, &chain, &evaluate).unwrap();
    assert_eq!(
        check_proof(&mut ctx, &chain, &u8_eq(applied, Term::U8(4))),
        Ok(())
    );
}

#[test]
fn pointwise_agreement_does_not_prove_function_equality() {
    let mut definitions = Definitions::new();
    let first = declare_successor(&mut definitions);
    let second = declare_successor(&mut definitions);
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    assert!(!same(&Term::Fn(first), &Term::Fn(second)));

    // They do agree everywhere, and the kernel can prove that ...
    let pointwise = Term::forall(Type::U8, |x| {
        u8_eq(call(first, vec![x.clone()]), call(second, vec![x]))
    });
    let agreement = Proof::forall_intro(Type::U8, |x| {
        // first(x) == x + 1 == second(x)
        let first_unfolds = Proof::Definition(call(first, vec![x.clone()]));
        let second_unfolds = Proof::Definition(call(second, vec![x.clone()]));
        let second_folds = Proof::transport(
            second_unfolds,
            |hole| u8_eq(hole, call(second, vec![x.clone()])),
            Proof::Refl(call(second, vec![x.clone()])),
        );
        Proof::transport(
            second_folds,
            |hole| u8_eq(call(first, vec![x.clone()]), hole),
            first_unfolds,
        )
    });
    assert_eq!(check_proof(&mut ctx, &agreement, &pointwise), Ok(()));

    // ... but no rule turns that into equality of the functions.
    let h = ctx.assume(pointwise).unwrap();
    let functions_equal = Term::eq(u8_to_u8(), Term::Fn(first), Term::Fn(second));
    for attempt in [
        Proof::Refl(Term::Fn(first)),
        Proof::Refl(Term::Fn(second)),
        Proof::hyp(h),
    ] {
        assert!(check_proof(&mut ctx, &attempt, &functions_equal).is_err());
    }
}

#[test]
fn claims_with_different_propositions_are_different_values() {
    // struct Claim { proposition: Prop }
    let mut definitions = Definitions::new();
    let claim = definitions
        .declare_struct(&Type::Tuple(vec![Type::Prop]))
        .unwrap();
    let mut ctx = Context::with_definitions(Rc::new(definitions));

    let truth = u8_eq(Term::U8(1), Term::U8(1));
    let falsehood = u8_eq(Term::U8(1), Term::U8(2));
    let a = Term::Struct(claim, vec![truth.clone()]);
    let b = Term::Struct(claim, vec![falsehood.clone()]);

    // Both are executable values with the same, empty, representation ...
    for value in [&a, &b] {
        assert_eq!(
            infer_term(&mut ctx, value, Mode::Executable),
            Ok(Type::Struct(claim))
        );
    }
    // ... and different logical values.
    assert!(!same(&a, &b));
    let equal = Term::eq(Type::Struct(claim), a.clone(), b.clone());
    assert!(check_proof(&mut ctx, &Proof::Refl(a.clone()), &equal).is_err());

    // Projection computes, at type Prop, and gives the proposition back.
    let projected = Term::proj(a.clone(), 0);
    let step = Proof::Projection(projected.clone());
    assert_eq!(
        infer_proof(&mut ctx, &step),
        Ok(Term::eq(Type::Prop, projected.clone(), truth.clone()))
    );
    // So a proof of the stored proposition proves the projection.
    let back = symm(&mut ctx, &step).unwrap();
    let proves_projection = Proof::transport(back, |hole| hole, Proof::Refl(Term::U8(1)));
    assert_eq!(
        check_proof(&mut ctx, &proves_projection, &projected),
        Ok(())
    );

    // If the two values were equal, their propositions would be: equality
    // substitution applies to logical values, not to representations.
    let h = ctx.assume(equal).unwrap();
    let a_proj = Term::proj(a, 0);
    let projections_equal = Proof::transport(
        Proof::hyp(h),
        |hole| Term::eq(Type::Prop, a_proj.clone(), Term::proj(hole, 0)),
        Proof::Refl(a_proj.clone()),
    );
    let b_step = Proof::Projection(Term::proj(b, 0));
    let chain = trans(&mut ctx, &projections_equal, &b_step).unwrap();
    let a_back = symm(&mut ctx, &step).unwrap();
    let chain = trans(&mut ctx, &a_back, &chain).unwrap();
    assert_eq!(
        check_proof(&mut ctx, &chain, &Term::eq(Type::Prop, truth, falsehood)),
        Ok(())
    );
}

// --- Functions ----------------------------------------------------------------

#[test]
fn a_call_computes_by_its_defining_equation_and_literal_steps() {
    let mut definitions = Definitions::new();
    let successor = declare_successor(&mut definitions);
    let mut ctx = Context::with_definitions(Rc::new(definitions));

    let three = call(successor, vec![Term::U8(3)]);
    assert_eq!(infer_term(&mut ctx, &three, Mode::Executable), Ok(Type::U8));
    let unfolds = Proof::Definition(three.clone());
    assert_eq!(
        infer_proof(&mut ctx, &unfolds),
        Ok(u8_eq(three.clone(), add_one(Term::U8(3))))
    );
    let evaluates = Proof::Literal(add_one(Term::U8(3)));
    let both = trans(&mut ctx, &unfolds, &evaluates).unwrap();
    assert_eq!(
        check_proof(&mut ctx, &both, &u8_eq(three.clone(), Term::U8(4))),
        Ok(())
    );
    // Reflexivity alone does not: the kernel does not compute.
    assert!(
        check_proof(
            &mut ctx,
            &Proof::Refl(Term::U8(4)),
            &u8_eq(three, Term::U8(4))
        )
        .is_err()
    );

    // Nested calls unfold outermost first, until none is left.
    let n = Term::var(ctx.declare(Type::U8).unwrap());
    let twice = call(successor, vec![call(successor, vec![n.clone()])]);
    let start = Proof::Refl(twice.clone());
    let unfolded = unfold(&mut ctx, successor, &start).unwrap();
    let expanded = add_one(add_one(n));
    assert_eq!(
        infer_proof(&mut ctx, &unfolded),
        Ok(u8_eq(expanded.clone(), expanded))
    );
}

#[test]
fn a_lemma_is_a_function_returning_a_proof() {
    // math fn add_one_cong(a: u8, b: u8, h: @[a == b])
    //     -> @[a.wrapping_add(1) == b.wrapping_add(1)]
    let mut definitions = Definitions::new();
    let signature = Type::function(3, |params| match params {
        [] | [_] => Type::U8,
        [a, b] => Type::proof(u8_eq(a.clone(), b.clone())),
        [a, b, _] => Type::proof(u8_eq(add_one(a.clone()), add_one(b.clone()))),
        _ => unreachable!(),
    });
    let lemma = definitions
        .declare_fn(&signature, |params| {
            let (a, h) = (params[0].clone(), params[2].clone());
            let left = add_one(a.clone());
            Term::proof(Proof::transport(
                Proof::OfTerm(h),
                |hole| u8_eq(left.clone(), add_one(hole)),
                Proof::Refl(add_one(a)),
            ))
        })
        .unwrap();
    let mut ctx = Context::with_definitions(Rc::new(definitions));

    let x = Term::var(ctx.declare(Type::U8).unwrap());
    let y = Term::var(ctx.declare(Type::U8).unwrap());
    let xy = ctx.assume(u8_eq(x.clone(), y.clone())).unwrap();

    // Using the lemma is a call; its result type is instantiated at x and y.
    let used = call(
        lemma,
        vec![x.clone(), y.clone(), Term::proof(Proof::hyp(xy))],
    );
    let goal = u8_eq(add_one(x.clone()), add_one(y.clone()));
    assert_eq!(
        check_proof(&mut ctx, &Proof::OfTerm(used.clone()), &goal),
        Ok(())
    );

    // The proof argument is checked against the instantiated parameter type.
    let wrong = call(
        lemma,
        vec![y.clone(), x.clone(), Term::proof(Proof::hyp(xy))],
    );
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::OfTerm(wrong)),
        Err(KernelError::ProofMismatch { .. })
    ));
    let not_a_proof = call(lemma, vec![x.clone(), y, x]);
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::OfTerm(not_a_proof)),
        Err(KernelError::ProofExpected(_))
    ));
    // A lemma has no defining equation to unfold: proofs are irrelevant.
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::Definition(used.clone())),
        Err(KernelError::EqualityAtProofType(_))
    ));
    // A call to a lemma is ghost.
    assert!(matches!(
        infer_term(&mut ctx, &used, Mode::Executable),
        Err(KernelError::GhostTypeInExecutable(_))
    ));
}

#[test]
fn a_dependent_result_type_is_instantiated_at_the_call() {
    // math fn increment(n: u8) -> (out: u8, @[out == n.wrapping_add(1)])
    let mut definitions = Definitions::new();
    let result_for = |n: &Term| {
        let n = n.clone();
        Type::tuple(move |earlier| match earlier {
            [] => Some(Type::U8),
            [out] => Some(Type::proof(u8_eq(out.clone(), add_one(n.clone())))),
            _ => None,
        })
    };
    let signature = Type::function(1, |params| match params {
        [] => Type::U8,
        [n] => result_for(n),
        _ => unreachable!(),
    });
    let increment = definitions
        .declare_fn(&signature, |params| {
            let n = &params[0];
            Term::tuple(
                &result_for(n),
                vec![
                    add_one(n.clone()),
                    Term::proof(Proof::Refl(add_one(n.clone()))),
                ],
            )
        })
        .unwrap();
    let mut ctx = Context::with_definitions(Rc::new(definitions));

    let m = Term::var(ctx.declare(Type::U8).unwrap());
    let result = call(increment, vec![m.clone()]);
    assert_eq!(
        infer_term(&mut ctx, &result, Mode::Executable),
        Ok(result_for(&m))
    );
    let evidence = Proof::OfTerm(Term::proj(result.clone(), 1));
    assert_eq!(
        infer_proof(&mut ctx, &evidence),
        Ok(u8_eq(Term::proj(result, 0), add_one(m)))
    );
}

#[test]
fn functions_are_values_and_may_be_parameters() {
    // math fn apply(f: math fn(u8) -> u8, x: u8) -> u8 { f(x) }
    let mut definitions = Definitions::new();
    let successor = declare_successor(&mut definitions);
    let is_three = declare_is_three(&mut definitions);
    let signature = Type::function(2, |params| match params {
        [] => u8_to_u8(),
        _ => Type::U8,
    });
    let apply = definitions
        .declare_fn(&signature, |params| {
            Term::call(params[0].clone(), vec![params[1].clone()])
        })
        .unwrap();
    let mut ctx = Context::with_definitions(Rc::new(definitions));

    let applied = call(apply, vec![Term::Fn(successor), Term::U8(3)]);
    assert_eq!(
        infer_term(&mut ctx, &applied, Mode::Executable),
        Ok(Type::U8)
    );
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Definition(applied.clone())),
        Ok(u8_eq(applied, call(successor, vec![Term::U8(3)])))
    );
    // A function value is executable when its result is; a predicate is not.
    assert_eq!(
        infer_term(&mut ctx, &Term::Fn(successor), Mode::Executable),
        Ok(u8_to_u8())
    );
    assert!(matches!(
        infer_term(&mut ctx, &Term::Fn(is_three), Mode::Executable),
        Err(KernelError::GhostTypeInExecutable(_))
    ));
    assert!(infer_term(&mut ctx, &Term::Fn(is_three), Mode::Logical).is_ok());
}

#[test]
fn declarations_are_acyclic_and_calls_are_checked() {
    let mut definitions = Definitions::new();
    // A body can only name functions that already exist, so it cannot name
    // itself: the identity it would need has not been issued.
    let future = {
        let mut elsewhere = Definitions::new();
        declare_successor(&mut elsewhere)
    };
    let recursive =
        definitions.declare_fn(&u8_to_u8(), |params| call(future, vec![params[0].clone()]));
    assert_eq!(recursive, Err(KernelError::UnknownFunction));

    // The body must have the declared result type.
    let wrong_body = definitions.declare_fn(&u8_to_u8(), |_| Term::Bool(true));
    assert!(matches!(wrong_body, Err(KernelError::TypeMismatch { .. })));
    assert!(matches!(
        definitions.declare_fn(&Type::U8, |_| Term::U8(0)),
        Err(KernelError::NotAFunction(_))
    ));

    let successor = declare_successor(&mut definitions);
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    assert_eq!(
        infer_term(&mut ctx, &call(successor, vec![]), Mode::Logical),
        Err(KernelError::FieldCount {
            expected: 1,
            found: 0
        })
    );
    assert!(matches!(
        infer_term(
            &mut ctx,
            &call(successor, vec![Term::Bool(true)]),
            Mode::Logical
        ),
        Err(KernelError::TypeMismatch { .. })
    ));
    assert!(matches!(
        infer_term(&mut ctx, &Term::call(Term::U8(1), vec![]), Mode::Logical),
        Err(KernelError::NotAFunction(Type::U8))
    ));
    // Only a call to a declared function has a defining equation.
    let f = Term::var(ctx.declare(u8_to_u8()).unwrap());
    assert!(matches!(
        infer_proof(
            &mut ctx,
            &Proof::Definition(Term::call(f, vec![Term::U8(1)]))
        ),
        Err(KernelError::NoComputationStep(_))
    ));
}

#[test]
fn rewrite_replaces_every_closed_occurrence() {
    let mut ctx = Context::new();
    let a = Term::var(ctx.declare(Type::U8).unwrap());
    let b = Term::var(ctx.declare(Type::U8).unwrap());
    let ab = ctx.assume(u8_eq(a.clone(), b.clone())).unwrap();
    let fact = ctx
        .assume(u8_eq(
            add_one(a.clone()),
            Term::op(Op::WrappingAdd, MachineInt::U8, vec![a.clone(), a]),
        ))
        .unwrap();

    let rewritten = rewrite(&mut ctx, &Proof::hyp(ab), &Proof::hyp(fact)).unwrap();
    assert_eq!(
        infer_proof(&mut ctx, &rewritten),
        Ok(u8_eq(
            add_one(b.clone()),
            Term::op(Op::WrappingAdd, MachineInt::U8, vec![b.clone(), b])
        ))
    );
    // Nothing to unfold is reported, not silently accepted.
    let mut definitions = Definitions::new();
    let successor = declare_successor(&mut definitions);
    let mut other = Context::with_definitions(Rc::new(definitions));
    assert!(matches!(
        unfold(&mut other, successor, &Proof::Refl(Term::U8(1))),
        Err(KernelError::NoComputationStep(_))
    ));
}

#[test]
fn unfold_and_fold_reach_calls_under_binders() {
    let mut definitions = Definitions::new();
    let is_three = declare_is_three(&mut definitions);
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    let claim = |x: Term| call(is_three, vec![x]);
    let body = |x: Term| u8_eq(x, Term::U8(3));

    // forall x { is_three(x) => is_three(x) }: the calls mention x.
    let folded = Term::forall(Type::U8, |x| Term::implies(claim(x.clone()), claim(x)));
    let unfolded = Term::forall(Type::U8, |x| Term::implies(body(x.clone()), body(x)));
    let h = ctx.assume(folded.clone()).unwrap();
    let opened = unfold(&mut ctx, is_three, &Proof::hyp(h)).unwrap();
    assert_eq!(check_proof(&mut ctx, &opened, &unfolded), Ok(()));
    let closed = fold(&mut ctx, is_three, &opened, &folded).unwrap();
    assert_eq!(check_proof(&mut ctx, &closed, &folded), Ok(()));

    // exists x { is_three(x) }
    let some_folded = Term::exists(Type::U8, claim);
    let some_unfolded = Term::exists(Type::U8, body);
    let e = ctx.assume(some_folded.clone()).unwrap();
    let opened = unfold(&mut ctx, is_three, &Proof::hyp(e)).unwrap();
    assert_eq!(check_proof(&mut ctx, &opened, &some_unfolded), Ok(()));
    let closed = fold(&mut ctx, is_three, &opened, &some_folded).unwrap();
    assert_eq!(check_proof(&mut ctx, &closed, &some_folded), Ok(()));

    // A closed call and a bound one together, two binders deep.
    let n = Term::var(ctx.declare(Type::U8).unwrap());
    let mixed = Term::implies(
        claim(n.clone()),
        Term::forall(Type::U8, |x| {
            Term::exists(Type::U8, |y| Term::implies(claim(x.clone()), claim(y)))
        }),
    );
    let mixed_unfolded = Term::implies(
        body(n),
        Term::forall(Type::U8, |x| {
            Term::exists(Type::U8, |y| Term::implies(body(x.clone()), body(y)))
        }),
    );
    let m = ctx.assume(mixed.clone()).unwrap();
    let opened = unfold(&mut ctx, is_three, &Proof::hyp(m)).unwrap();
    assert_eq!(check_proof(&mut ctx, &opened, &mixed_unfolded), Ok(()));
    let closed = fold(&mut ctx, is_three, &opened, &mixed).unwrap();
    assert_eq!(check_proof(&mut ctx, &closed, &mixed), Ok(()));
    // Nothing is left behind in the context by the descent.
    assert!(check_proof(&mut ctx, &Proof::hyp(m), &mixed).is_ok());
}

#[test]
fn a_function_that_needs_a_ghost_to_compute_has_no_runtime_form() {
    // math fn narrow(n: Int) -> u8 { wrap[u8](n) }: a fine logical function,
    // but a call to it would turn an erased number into a byte.
    let mut definitions = Definitions::new();
    let int_to_u8 = Type::function(1, |params| match params {
        [] => Type::Int,
        _ => Type::U8,
    });
    let narrow = definitions
        .declare_fn(&int_to_u8, |params| {
            Term::wrap(MachineInt::U8, params[0].clone())
        })
        .unwrap();
    // The restriction survives function boundaries: a caller is logical-only
    // too, even though its own signature is all executable data.
    let through = definitions
        .declare_fn(&u8_to_u8(), |params| {
            call(narrow, vec![Term::view(MachineInt::U8, params[0].clone())])
        })
        .unwrap();
    let successor = declare_successor(&mut definitions);
    // An `Int` parameter that the result does not depend on does no harm.
    let ignores = definitions.declare_fn(&int_to_u8, |_| Term::U8(7)).unwrap();
    assert!(!definitions.is_executable(narrow));
    assert!(!definitions.is_executable(through));
    assert!(definitions.is_executable(successor));
    assert!(definitions.is_executable(ignores));

    let mut ctx = Context::with_definitions(Rc::new(definitions));
    let n = Term::var(ctx.declare_ghost(Type::Int).unwrap());
    let x = Term::var(ctx.declare(Type::U8).unwrap());
    for (term, allowed) in [
        (call(narrow, vec![n.clone()]), false),
        (call(narrow, vec![Term::int(1)]), false),
        (call(through, vec![x.clone()]), false),
        (Term::Fn(narrow), false),
        (call(ignores, vec![n.clone()]), true),
        (call(successor, vec![x.clone()]), true),
    ] {
        assert_eq!(
            infer_term(&mut ctx, &term, Mode::Executable).is_ok(),
            allowed,
            "{term}"
        );
        // Every one of them is an ordinary logical term.
        assert!(infer_term(&mut ctx, &term, Mode::Logical).is_ok());
    }
    assert_eq!(
        infer_term(&mut ctx, &call(narrow, vec![n]), Mode::Executable),
        Err(KernelError::LogicalFunctionInExecutable)
    );
    // Logic still computes with it: narrow(0) and narrow(1) differ, which is
    // exactly why no erased program may depend on the call.
    for k in [0u8, 1] {
        let applied = call(narrow, vec![Term::int(i64::from(k))]);
        assert_eq!(
            infer_proof(&mut ctx, &Proof::Evaluate(applied.clone())),
            Ok(u8_eq(applied, Term::U8(k)))
        );
    }
}
