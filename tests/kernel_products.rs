//! Acceptance tests for kernel gate K2 (the kernel contract in atlas.html): tuples, structs,
//! dependent proof fields, the let, projection, and literal computation
//! axioms, and proof irrelevance in comparison.
//! Every term here is written by hand; nothing comes from the parser.

use std::rc::Rc;

use locus::kernel::{
    Context, Definitions, KernelError, MachineInt, Mode, Op, Proof, StructId, Term, Type,
    check_proof, check_type, infer_proof, infer_term, same,
};

fn u8_eq(left: Term, right: Term) -> Term {
    Term::eq(Type::U8, left, right)
}

fn add_one(term: Term) -> Term {
    Term::op(Op::WrappingAdd, MachineInt::U8, vec![term, Term::U8(1)])
}

/// `(out: u8, @[out == n.wrapping_add(1)])`
fn increment_result(n: &Term) -> Type {
    Type::tuple(|earlier| match earlier {
        [] => Some(Type::U8),
        [out] => Some(Type::proof(u8_eq(out.clone(), add_one(n.clone())))),
        _ => None,
    })
}

/// `struct Sum { a: u8, b: u8, evidence: @[a.wrapping_add(b) == 10] }`.
/// Any closed instance of its proof obligation is decidable by the literal
/// axiom, which makes it a convenient stand-in for `NonZero` until K5 gives
/// the kernel an ordering on u8.
fn declare_sum(definitions: &mut Definitions) -> StructId {
    let fields = Type::tuple(|earlier| match earlier {
        [] | [_] => Some(Type::U8),
        [a, b] => Some(Type::proof(u8_eq(
            Term::op(Op::WrappingAdd, MachineInt::U8, vec![a.clone(), b.clone()]),
            Term::U8(10),
        ))),
        _ => None,
    });
    definitions.declare_struct(&fields).unwrap()
}

fn sum_value(id: StructId, a: u8, b: u8) -> Term {
    let evidence = Proof::Literal(Term::op(
        Op::WrappingAdd,
        MachineInt::U8,
        vec![Term::U8(a), Term::U8(b)],
    ));
    Term::Struct(id, vec![Term::U8(a), Term::U8(b), Term::proof(evidence)])
}

// --- The stated gate conditions ---------------------------------------------

#[test]
#[doc = "spec: 2.1:7, 2.4:1, 2.23:3"]
fn a_dependent_data_and_proof_result_checks() {
    // fn increment(n: u8) -> (out: u8, @[out == n.wrapping_add(1)]) {
    //     let out = n.wrapping_add(1);
    //     (out, _)
    // }
    let mut ctx = Context::new();
    let n = Term::var(ctx.declare(Type::U8).unwrap());
    let (out, out_equation) = ctx.define(&add_one(n.clone())).unwrap();
    let result_type = increment_result(&n);

    let value = Term::tuple(
        &result_type,
        vec![Term::var(out), Term::proof(Proof::hyp(out_equation))],
    );
    // The whole result is executable data; its proof field is not.
    assert_eq!(
        infer_term(&mut ctx, &value, Mode::Executable),
        Ok(result_type.clone())
    );

    // Reflexivity does not prove the field: the kernel never unfolds `out`.
    let by_refl = Term::tuple(
        &result_type,
        vec![Term::var(out), Term::proof(Proof::Refl(Term::var(out)))],
    );
    assert!(matches!(
        infer_term(&mut ctx, &by_refl, Mode::Executable),
        Err(KernelError::ProofMismatch { .. })
    ));
}

#[test]
fn returned_data_instantiates_a_later_proof_field() {
    // let r = increment(n);  then r.1 proves r.0 == n.wrapping_add(1)
    let mut ctx = Context::new();
    let n = Term::var(ctx.declare(Type::U8).unwrap());
    let r = Term::var(ctx.declare(increment_result(&n)).unwrap());

    let data = Term::proj(r.clone(), 0);
    let evidence = Proof::OfTerm(Term::proj(r.clone(), 1));
    assert_eq!(
        infer_proof(&mut ctx, &evidence),
        Ok(u8_eq(data.clone(), add_one(n.clone())))
    );

    // The caller can reuse it: (r.0).wrapping_add(1) == n + 1 + 1.
    let left = add_one(data.clone());
    let reused = Proof::transport(
        evidence,
        |hole| u8_eq(left.clone(), add_one(hole)),
        Proof::Refl(add_one(data.clone())),
    );
    let goal = u8_eq(add_one(data), add_one(add_one(n)));
    assert_eq!(check_proof(&mut ctx, &reused, &goal), Ok(()));
}

#[test]
#[doc = "spec: 2.19:2, 2.19:3"]
fn equal_data_with_different_proofs_is_equal_without_a_proof_step() {
    let mut definitions = Definitions::new();
    let sum = declare_sum(&mut definitions);
    let mut ctx = Context::with_definitions(Rc::new(definitions));

    // Two proofs of 3.wrapping_add(7) == 10: the literal axiom, and the same
    // fact routed through a hypothesis-free detour.
    let direct = Proof::Literal(Term::op(
        Op::WrappingAdd,
        MachineInt::U8,
        vec![Term::U8(3), Term::U8(7)],
    ));
    let target = Term::op(
        Op::WrappingAdd,
        MachineInt::U8,
        vec![Term::U8(3), Term::U8(7)],
    );
    let detour = Proof::transport(
        Proof::Refl(Term::U8(10)),
        |hole| u8_eq(target.clone(), hole),
        direct.clone(),
    );
    let first = Term::Struct(sum, vec![Term::U8(3), Term::U8(7), Term::proof(direct)]);
    let second = Term::Struct(sum, vec![Term::U8(3), Term::U8(7), Term::proof(detour)]);
    assert_ne!(first, second);
    assert!(same(&first, &second));

    // So reflexivity on one proves equality with the other.
    let goal = Term::eq(Type::Struct(sum), first.clone(), second);
    assert_eq!(check_proof(&mut ctx, &Proof::Refl(first), &goal), Ok(()));

    // Different data is still different.
    assert!(!same(&sum_value(sum, 3, 7), &sum_value(sum, 4, 6)));
}

// --- Computation axioms -------------------------------------------------------

#[test]
#[doc = "spec: 2.1:7, 2.4:1, 2.5:2"]
fn literal_arithmetic_is_an_explicit_step() {
    let mut ctx = Context::new();
    let one_plus_one = add_one(Term::U8(1));
    let step = Proof::Literal(one_plus_one.clone());
    assert_eq!(
        check_proof(&mut ctx, &step, &u8_eq(one_plus_one, Term::U8(2))),
        Ok(())
    );

    let wraps = Term::op(
        Op::WrappingAdd,
        MachineInt::U8,
        vec![Term::U8(255), Term::U8(1)],
    );
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Literal(wraps.clone())),
        Ok(u8_eq(wraps, Term::U8(0)))
    );
    let borrows = Term::op(
        Op::WrappingSub,
        MachineInt::U8,
        vec![Term::U8(0), Term::U8(1)],
    );
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Literal(borrows.clone())),
        Ok(u8_eq(borrows, Term::U8(255)))
    );

    // Only literals evaluate.
    let n = Term::var(ctx.declare(Type::U8).unwrap());
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::Literal(add_one(n))),
        Err(KernelError::NoComputationStep(_))
    ));
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::Literal(Term::U8(3))),
        Err(KernelError::NoComputationStep(_))
    ));
}

#[test]
fn a_false_literal_claim_stays_unprovable() {
    let mut ctx = Context::new();
    let sum = add_one(Term::U8(1));
    let goal = u8_eq(sum.clone(), Term::U8(3));
    assert!(matches!(
        check_proof(&mut ctx, &Proof::Literal(sum), &goal),
        Err(KernelError::ProofMismatch { .. })
    ));
}

#[test]
#[doc = "spec: 2.1:7, 2.4:1, 2.5:2, 2.18:2"]
fn projection_from_a_known_constructor_is_an_explicit_step() {
    let mut ctx = Context::new();
    let pair_type = Type::Tuple(vec![Type::U8, Type::Bool]);
    let pair = Term::tuple(&pair_type, vec![Term::U8(3), Term::Bool(true)]);

    let first = Term::proj(pair.clone(), 0);
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Projection(first.clone())),
        Ok(u8_eq(first.clone(), Term::U8(3)))
    );
    let second = Term::proj(pair, 1);
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Projection(second.clone())),
        Ok(Term::eq(Type::Bool, second, Term::Bool(true)))
    );
    // Without the step, the projection is just a term.
    assert!(
        check_proof(
            &mut ctx,
            &Proof::Refl(Term::U8(3)),
            &u8_eq(first, Term::U8(3))
        )
        .is_err()
    );

    // A projection from a variable does not reduce.
    let r = Term::var(ctx.declare(pair_type).unwrap());
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::Projection(Term::proj(r, 0))),
        Err(KernelError::NoComputationStep(_))
    ));
}

#[test]
#[doc = "spec: 2.2:4"]
fn the_let_axiom_is_declare_plus_assume() {
    let mut ctx = Context::new();
    let n = Term::var(ctx.declare(Type::U8).unwrap());
    let (out, equation) = ctx.define(&add_one(n.clone())).unwrap();
    assert_eq!(
        infer_proof(&mut ctx, &Proof::hyp(equation)),
        Ok(u8_eq(Term::var(out), add_one(n)))
    );
    // A let of executable data is executable; a let of ghost data is ghost.
    assert_eq!(
        infer_term(&mut ctx, &Term::var(out), Mode::Executable),
        Ok(Type::U8)
    );
    let k = Term::var(ctx.declare_ghost(Type::U8).unwrap());
    let (ghost_let, _) = ctx.define(&add_one(k)).unwrap();
    assert_eq!(
        infer_term(&mut ctx, &Term::var(ghost_let), Mode::Executable),
        Err(KernelError::GhostInExecutable(ghost_let))
    );
    // The same fact is available without the context helper: instantiate
    // "forall x, x == e => P(x)" at e and discharge the premise by refl.
    let e = add_one(Term::U8(1));
    let general = Proof::forall_intro(Type::U8, |x| {
        Proof::implies_intro(u8_eq(x.clone(), e.clone()), |h| h)
    });
    let used = Proof::implies_elim(
        Proof::forall_elim(general, e.clone()),
        Proof::Refl(e.clone()),
    );
    assert_eq!(infer_proof(&mut ctx, &used), Ok(u8_eq(e.clone(), e)));
}

// --- Products and ghost rules -------------------------------------------------

#[test]
#[doc = "spec: 2.4:6, 2.4:7, 2.4:8"]
fn a_proof_field_is_a_logical_position_inside_executable_data() {
    let mut ctx = Context::new();
    let n = Term::var(ctx.declare(Type::U8).unwrap());
    let k = Term::var(ctx.declare_ghost(Type::U8).unwrap());
    let k_is_n = ctx.assume(u8_eq(n.clone(), k.clone())).unwrap();

    // (value: u8, @[value == k]) with value := n: the ghost k appears only in
    // the proposition, so the tuple is executable.
    let tagged = Type::tuple(|earlier| match earlier {
        [] => Some(Type::U8),
        [value] => Some(Type::proof(u8_eq(value.clone(), k.clone()))),
        _ => None,
    });
    let good = Term::tuple(&tagged, vec![n.clone(), Term::proof(Proof::hyp(k_is_n))]);
    assert!(infer_term(&mut ctx, &good, Mode::Executable).is_ok());

    // Storing the ghost in the data field is not.
    let leaky = Term::tuple(
        &tagged,
        vec![k.clone(), Term::proof(Proof::Refl(k.clone()))],
    );
    assert!(matches!(
        infer_term(&mut ctx, &leaky, Mode::Executable),
        Err(KernelError::GhostInExecutable(_))
    ));
    assert!(infer_term(&mut ctx, &leaky, Mode::Logical).is_ok());
}

#[test]
fn a_proof_field_cannot_be_projected_into_executable_data() {
    let mut ctx = Context::new();
    let n = Term::var(ctx.declare(Type::U8).unwrap());
    let r = Term::var(ctx.declare(increment_result(&n)).unwrap());

    assert_eq!(
        infer_term(&mut ctx, &Term::proj(r.clone(), 0), Mode::Executable),
        Ok(Type::U8)
    );
    assert!(matches!(
        infer_term(&mut ctx, &Term::proj(r.clone(), 1), Mode::Executable),
        Err(KernelError::GhostTypeInExecutable(_))
    ));
    assert!(matches!(
        infer_term(&mut ctx, &Term::proj(r, 2), Mode::Logical),
        Err(KernelError::NoSuchField {
            index: 2,
            fields: 2
        })
    ));
    assert!(matches!(
        infer_term(&mut ctx, &Term::proj(n, 0), Mode::Logical),
        Err(KernelError::NotAProduct(Type::U8))
    ));
}

#[test]
#[doc = "spec: 2.1:7, 2.4:1, 2.4:9, 2.19:5"]
fn a_proof_field_must_be_a_checked_proof_of_the_instantiated_claim() {
    let mut definitions = Definitions::new();
    let sum = declare_sum(&mut definitions);
    let mut ctx = Context::with_definitions(Rc::new(definitions));

    assert_eq!(
        infer_term(&mut ctx, &sum_value(sum, 3, 7), Mode::Executable),
        Ok(Type::Struct(sum))
    );
    // 3 + 8 is 11: the literal axiom proves the wrong equation for the field.
    assert!(matches!(
        infer_term(&mut ctx, &sum_value(sum, 3, 8), Mode::Executable),
        Err(KernelError::ProofMismatch { .. })
    ));
    // A proof field cannot be filled with data, and a data field cannot be
    // filled with a proof.
    let data_for_proof = Term::Struct(sum, vec![Term::U8(3), Term::U8(7), Term::U8(10)]);
    assert!(matches!(
        infer_term(&mut ctx, &data_for_proof, Mode::Logical),
        Err(KernelError::ProofExpected(_))
    ));
    let too_few = Term::Struct(sum, vec![Term::U8(3), Term::U8(7)]);
    assert_eq!(
        infer_term(&mut ctx, &too_few, Mode::Logical),
        Err(KernelError::FieldCount {
            expected: 3,
            found: 2
        })
    );
}

#[test]
#[doc = "spec: 2.1:5, 2.3:1"]
fn structs_are_nominal_and_tuples_are_structural() {
    let mut definitions = Definitions::new();
    let first = declare_sum(&mut definitions);
    let second = declare_sum(&mut definitions);
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    assert_ne!(first, second);

    let value = sum_value(first, 3, 7);
    let goal = Term::eq(Type::Struct(second), value.clone(), value.clone());
    assert!(matches!(
        check_proof(&mut ctx, &Proof::Refl(value), &goal),
        Err(KernelError::TypeMismatch { .. })
    ));

    // Tuple binder names are not part of the type: two builders, one type.
    let n = Term::var(ctx.declare(Type::U8).unwrap());
    assert_eq!(increment_result(&n), increment_result(&n));
}

#[test]
fn ill_formed_product_types_are_rejected() {
    let mut ctx = Context::new();
    // A field that refers to a later field: a dangling index.
    let forward = Type::Tuple(vec![
        Type::proof(u8_eq(Term::Bound(0), Term::U8(0))),
        Type::U8,
    ]);
    assert_eq!(
        check_type(&mut ctx, &forward),
        Err(KernelError::DanglingBound)
    );
    assert!(ctx.declare(forward).is_err());

    // A proof type over something that is not a proposition.
    assert!(check_type(&mut ctx, &Type::proof(Term::U8(1))).is_err());

    // A struct declaration is closed: it cannot mention a context variable.
    let n = Term::var(ctx.declare(Type::U8).unwrap());
    let mut definitions = Definitions::new();
    let open = Type::Tuple(vec![Type::U8, Type::proof(u8_eq(Term::Bound(0), n))]);
    assert!(matches!(
        definitions.declare_struct(&open),
        Err(KernelError::UnknownVariable(_))
    ));
    assert!(matches!(
        definitions.declare_struct(&Type::U8),
        Err(KernelError::NotAProduct(_))
    ));
    // An undeclared struct is not a type.
    assert!(
        check_type(
            &mut Context::new(),
            &Type::Struct(declare_sum(&mut definitions))
        )
        .is_err()
    );
}

#[test]
#[doc = "spec: 2.4:11"]
fn equality_between_proofs_is_not_a_proposition() {
    let mut ctx = Context::new();
    let claim = u8_eq(Term::U8(1), Term::U8(1));
    let proof = Term::proof(Proof::Refl(Term::U8(1)));
    let proof_type = Type::proof(claim);

    let equation = Term::eq(proof_type, proof.clone(), proof.clone());
    assert!(matches!(
        infer_term(&mut ctx, &equation, Mode::Logical),
        Err(KernelError::EqualityAtProofType(_))
    ));
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::Refl(proof)),
        Err(KernelError::EqualityAtProofType(_))
    ));
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::OfTerm(Term::U8(1))),
        Err(KernelError::NotAProofType(Type::U8))
    ));
}

#[test]
#[doc = "spec: 2.1:4, 2.1:8"]
fn a_later_field_may_depend_on_two_earlier_fields_under_a_quantifier() {
    // (a: u8, b: u8, @[forall x { a == x => b == x => a == b }]) exercises
    // telescope indices beneath another binder.
    let mut ctx = Context::new();
    let ty = Type::tuple(|earlier| match earlier {
        [] | [_] => Some(Type::U8),
        [a, b] => Some(Type::proof(Term::forall(Type::U8, |x| {
            Term::implies(
                u8_eq(a.clone(), x.clone()),
                Term::implies(u8_eq(b.clone(), x), u8_eq(a.clone(), b.clone())),
            )
        }))),
        _ => None,
    });
    assert_eq!(check_type(&mut ctx, &ty), Ok(()));

    let p = Term::var(ctx.declare(Type::U8).unwrap());
    let q = Term::var(ctx.declare(Type::U8).unwrap());
    // a == x and b == x give a == b: rewrite x to b in "a == x" along x == b.
    let evidence = Proof::forall_intro(Type::U8, |x| {
        Proof::implies_intro(u8_eq(p.clone(), x.clone()), |a_is_x| {
            Proof::implies_intro(u8_eq(q.clone(), x.clone()), |b_is_x| {
                let x_is_b = Proof::transport(
                    b_is_x,
                    |hole| u8_eq(hole, q.clone()),
                    Proof::Refl(q.clone()),
                );
                Proof::transport(x_is_b, |hole| u8_eq(p.clone(), hole), a_is_x)
            })
        })
    });
    let value = Term::tuple(&ty, vec![p.clone(), q.clone(), Term::proof(evidence)]);
    assert_eq!(infer_term(&mut ctx, &value, Mode::Executable), Ok(ty));
}

#[test]
#[doc = "spec: 2.18:3"]
fn projection_computes_through_a_nested_dependent_product() {
    // (a: u8, inner: (b: u8, @[b == a])): the inner type mentions the outer
    // field. Projecting the inner product out of a literal value is typed by
    // that value's own fields, so the projection axiom applies.
    let mut ctx = Context::new();
    let inner_for = |a: Term| {
        Type::tuple(move |earlier| match earlier {
            [] => Some(Type::U8),
            [b] => Some(Type::proof(u8_eq(b.clone(), a.clone()))),
            _ => None,
        })
    };
    let outer = Type::tuple(|earlier| match earlier {
        [] => Some(Type::U8),
        [a] => Some(inner_for(a.clone())),
        _ => None,
    });
    let three = Term::U8(3);
    let inner_value = Term::tuple(
        &inner_for(three.clone()),
        vec![three.clone(), Term::proof(Proof::Refl(three.clone()))],
    );
    let value = Term::tuple(&outer, vec![three.clone(), inner_value.clone()]);
    assert_eq!(
        infer_term(&mut ctx, &value, Mode::Executable),
        Ok(outer.clone())
    );

    let projected = Term::proj(value, 1);
    assert_eq!(
        infer_term(&mut ctx, &projected, Mode::Logical),
        Ok(inner_for(three.clone()))
    );
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Projection(projected.clone())),
        Ok(Term::eq(inner_for(three), projected, inner_value))
    );
    // From a variable, earlier fields are still named by projection.
    let r = Term::var(ctx.declare(outer).unwrap());
    assert_eq!(
        infer_term(&mut ctx, &Term::proj(r.clone(), 1), Mode::Logical),
        Ok(inner_for(Term::proj(r, 0)))
    );
}
