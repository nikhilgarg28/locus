use locus::kernel::{
    Context, Definitions, KernelError, Mode, Proof, Term, Type, check_proof, infer_term,
};
use std::rc::Rc;
#[test]
#[doc = "spec: 2.36:1, 2.36:2"]
fn box_snapshot_projection_is_checked() {
    let mut ctx = Context::new();
    let boxed = Term::Boxed(Box::new(Term::U8(9)));
    assert_eq!(
        infer_term(&mut ctx, &boxed, Mode::Logical),
        Ok(Type::Boxed(Box::new(Type::U8)))
    );
    assert!(infer_term(&mut ctx, &boxed, Mode::Executable).is_err());
    let projected = Term::proj(boxed.clone(), 0);
    check_proof(
        &mut ctx,
        &Proof::Projection(projected.clone()),
        &Term::eq(Type::U8, projected, Term::U8(9)),
    )
    .unwrap();
    assert!(infer_term(&mut ctx, &Term::proj(boxed, 1), Mode::Logical).is_err());
}
#[test]
fn logical_payload_does_not_make_box_logical() {
    let (mut defs, _) = Definitions::with_prelude();
    let ty = Type::Boxed(Box::new(Type::Int));
    assert!(!defs.is_logical_type(&ty));
    assert!(!defs.is_erased_type(&ty));
    let wrapper = defs.declare_struct(&Type::Tuple(vec![ty])).unwrap();
    assert!(defs.mark_logical(&Type::Struct(wrapper)).is_err());
}
#[test]
fn runtime_recursive_models_only_descend_through_boxed_children() {
    let (mut defs, _) = Definitions::with_prelude();
    let id = defs
        .declare_runtime_enum_group(1, |ids| {
            vec![vec![
                Type::Tuple(vec![]),
                Type::Tuple(vec![Type::Boxed(Box::new(Type::Enum(ids[0])))]),
            ]]
        })
        .unwrap()[0];
    let signature = Type::Fn(vec![Type::Enum(id)], Box::new(Type::Int));
    let count = defs
        .declare_structural_fn(&signature, 0, |f, args| {
            Term::case(
                args[0].clone(),
                Type::Int,
                vec![
                    (0, Box::new(|_, _| Term::int(0))),
                    (
                        1,
                        Box::new(move |fields, _| {
                            Term::int_add(
                                Term::int(1),
                                Term::call(Term::Fn(f), vec![Term::proj(fields[0].clone(), 0)]),
                            )
                        }),
                    ),
                ],
            )
        })
        .unwrap();
    let value = Term::Variant(
        id,
        1,
        vec![Term::Boxed(Box::new(Term::Variant(id, 0, vec![])))],
    );
    let call = Term::call(Term::Fn(count), vec![value]);
    let mut ctx = Context::with_definitions(Rc::new(defs.clone()));
    check_proof(
        &mut ctx,
        &Proof::Evaluate(call.clone()),
        &Term::eq(Type::Int, call, Term::int(1)),
    )
    .unwrap();
    assert!(matches!(
        defs.declare_structural_fn(&signature, 0, |f, args| Term::call(
            Term::Fn(f),
            vec![args[0].clone()]
        )),
        Err(KernelError::InvalidRecursion(_))
    ));
}
#[test]
#[doc = "spec: 2.36:3"]
fn unboxed_or_negative_runtime_recursion_is_rejected_atomically() {
    let (mut defs, _) = Definitions::with_prelude();
    for negative in [false, true] {
        let rejected = defs.declare_runtime_enum_group(1, |ids| {
            let recursive = Type::Enum(ids[0]);
            vec![vec![Type::Tuple(vec![if negative {
                Type::Boxed(Box::new(Type::Fn(vec![recursive], Box::new(Type::Int))))
            } else {
                recursive
            }])]]
        });
        assert!(rejected.is_err());
    }
    assert!(
        defs.declare_runtime_enum_group(1, |_| vec![vec![Type::Tuple(vec![])]])
            .is_ok()
    );
}
