//! Immutable collection snapshots have checked bounds and no runtime authority.
use locus::kernel::buffer::{bounds, length};
use locus::kernel::{
    BufferOp, Context, KernelError, Mode, Proof, Term, Type, check_proof, infer_proof, infer_term,
};

fn literal(values: &[u8]) -> Term {
    Term::Buffer {
        op: BufferOp::Literal,
        element: Type::U8,
        arguments: values.iter().copied().map(Term::U8).collect(),
    }
}
fn access(ctx: &mut Context, value: Term, index: Term, replacement: Option<Term>) -> Term {
    let mut arguments = vec![value.clone(), index.clone()];
    for claim in bounds(&Type::U8, &value, &index) {
        let hyp = ctx.assume(claim).unwrap();
        arguments.push(Term::proof(Proof::hyp(hyp)));
    }
    let op = if let Some(value) = replacement {
        arguments.push(value);
        BufferOp::Set
    } else {
        BufferOp::Get
    };
    Term::Buffer {
        op,
        element: Type::U8,
        arguments,
    }
}

#[test]
#[doc = "spec: 2.35:1, 2.35:2, 2.35:3, 2.35:4, 2.35:7"]
fn literal_length_and_reads_have_explicit_steps() {
    let mut ctx = Context::new();
    let values = literal(&[3, 7]);
    assert_eq!(
        infer_term(&mut ctx, &values, Mode::Logical),
        Ok(Type::Buffer(Box::new(Type::U8)))
    );
    let len = length(Type::U8, values.clone());
    check_proof(
        &mut ctx,
        &Proof::BufferStep(len.clone()),
        &Term::eq(Type::Int, len, Term::int(2)),
    )
    .unwrap();
    let read = access(&mut ctx, values, Term::int(1), None);
    check_proof(
        &mut ctx,
        &Proof::BufferStep(read.clone()),
        &Term::eq(Type::U8, read.clone(), Term::U8(7)),
    )
    .unwrap();
    assert!(
        check_proof(
            &mut ctx,
            &Proof::BufferStep(read.clone()),
            &Term::eq(Type::U8, read, Term::U8(3))
        )
        .is_err()
    );
}

#[test]
fn stale_bounds_do_not_authorize_a_different_snapshot() {
    let mut ctx = Context::new();
    let values = literal(&[1, 2]);
    let mut read = access(&mut ctx, values, Term::int(1), None);
    let Term::Buffer { arguments, .. } = &mut read else {
        unreachable!()
    };
    arguments[0] = literal(&[]);
    assert!(infer_term(&mut ctx, &read, Mode::Logical).is_err());
    let Term::Buffer { arguments, .. } = &mut read else {
        unreachable!()
    };
    arguments[0] = literal(&[1, 2]);
    arguments[1] = Term::int(2);
    assert!(infer_term(&mut ctx, &read, Mode::Logical).is_err());
}

#[test]
fn updates_preserve_length_but_change_the_read() {
    let mut ctx = Context::new();
    let before = literal(&[1, 2]);
    let after = access(&mut ctx, before.clone(), Term::int(1), Some(Term::U8(9)));
    let len_after = length(Type::U8, after.clone());
    check_proof(
        &mut ctx,
        &Proof::BufferStep(len_after.clone()),
        &Term::eq(Type::Int, len_after, length(Type::U8, before)),
    )
    .unwrap();
    let read = access(&mut ctx, after, Term::int(1), None);
    check_proof(
        &mut ctx,
        &Proof::BufferStep(read.clone()),
        &Term::eq(Type::U8, read, Term::U8(9)),
    )
    .unwrap();
}

#[test]
fn all_arguments_and_proof_positions_are_checked() {
    let mut ctx = Context::new();
    let malformed = Term::Buffer {
        op: BufferOp::Literal,
        element: Type::U8,
        arguments: vec![Term::int(1)],
    };
    assert!(infer_term(&mut ctx, &malformed, Mode::Logical).is_err());
    let mut read = access(&mut ctx, literal(&[3]), Term::int(0), None);
    let Term::Buffer { arguments, .. } = &mut read else {
        unreachable!()
    };
    arguments[2] = Term::proof(Proof::Refl(Term::U8(0)));
    assert!(infer_term(&mut ctx, &read, Mode::Logical).is_err());
    let claim = Term::eq(Type::U8, Term::U8(0), Term::U8(0));
    let id = ctx.declare(Type::proof(claim.clone())).unwrap();
    let noncanonical = Term::Buffer {
        op: BufferOp::Literal,
        element: Type::proof(claim),
        arguments: vec![Term::Free(id)],
    };
    assert!(matches!(
        infer_term(&mut ctx, &noncanonical, Mode::Logical),
        Err(KernelError::ProofExpected(_))
    ));
}

#[test]
fn buffer_terms_cannot_smuggle_runtime_reads_or_allocations() {
    let mut ctx = Context::new();
    let read = access(&mut ctx, literal(&[4]), Term::int(0), None);
    assert!(infer_term(&mut ctx, &read, Mode::Executable).is_err());
    assert!(infer_term(&mut ctx, &literal(&[4]), Mode::Executable).is_err());
    let values = Term::Free(ctx.declare(Type::Buffer(Box::new(Type::U8))).unwrap());
    for upper in [false, true] {
        infer_proof(
            &mut ctx,
            &Proof::BufferBound {
                value: values.clone(),
                upper,
            },
        )
        .unwrap();
    }
    assert!(
        infer_proof(
            &mut ctx,
            &Proof::BufferBound {
                value: Term::U8(0),
                upper: false
            }
        )
        .is_err()
    );
}

#[test]
fn push_needs_room_and_establishes_last_element() {
    let mut ctx = Context::new();
    let before = literal(&[1]);
    let room = ctx
        .assume(Term::int_lt(
            length(Type::U8, before.clone()),
            Term::Int(locus::kernel::MachineInt::U64.max()),
        ))
        .unwrap();
    let after = Term::Buffer {
        op: BufferOp::Push,
        element: Type::U8,
        arguments: vec![before.clone(), Term::U8(8), Term::proof(Proof::hyp(room))],
    };
    let read = access(&mut ctx, after.clone(), length(Type::U8, before), None);
    check_proof(
        &mut ctx,
        &Proof::BufferStep(read.clone()),
        &Term::eq(Type::U8, read, Term::U8(8)),
    )
    .unwrap();
    check_proof(
        &mut ctx,
        &Proof::BufferStep(after.clone()),
        &Term::eq(Type::Buffer(Box::new(Type::U8)), after, literal(&[1, 8])),
    )
    .unwrap();
}
#[test]
#[doc = "spec: 2.37:1"]
fn push_preserves_only_previously_bounded_reads() {
    let mut ctx = Context::new();
    let before = Term::Free(ctx.declare(Type::Buffer(Box::new(Type::U8))).unwrap());
    let room = ctx
        .assume(Term::int_lt(
            length(Type::U8, before.clone()),
            Term::Int(locus::kernel::MachineInt::U64.max()),
        ))
        .unwrap();
    let after = Term::Buffer {
        op: BufferOp::Push,
        element: Type::U8,
        arguments: vec![before.clone(), Term::U8(9), Term::proof(Proof::hyp(room))],
    };
    let before_read = access(&mut ctx, before.clone(), Term::int(0), None);
    let after_read = access(&mut ctx, after.clone(), Term::int(0), None);
    let equation = Term::eq(Type::U8, after_read.clone(), before_read.clone());
    check_proof(&mut ctx, &Proof::BufferStep(equation.clone()), &equation).unwrap();
    let wrong_index = access(&mut ctx, before.clone(), Term::int(1), None);
    assert!(
        infer_proof(
            &mut ctx,
            &Proof::BufferStep(Term::eq(Type::U8, after_read.clone(), wrong_index))
        )
        .is_err()
    );
    let other = Term::Free(ctx.declare(Type::Buffer(Box::new(Type::U8))).unwrap());
    let wrong_source = access(&mut ctx, other, Term::int(0), None);
    assert!(
        infer_proof(
            &mut ctx,
            &Proof::BufferStep(Term::eq(Type::U8, after_read.clone(), wrong_source))
        )
        .is_err()
    );
    let mut invalid_old = before_read;
    let Term::Buffer { arguments, .. } = &mut invalid_old else {
        unreachable!()
    };
    arguments[3] = Term::proof(Proof::Omitted);
    assert!(
        infer_proof(
            &mut ctx,
            &Proof::BufferStep(Term::eq(Type::U8, after_read, invalid_old))
        )
        .is_err()
    );
}
