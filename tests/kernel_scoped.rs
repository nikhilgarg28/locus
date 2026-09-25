//! Family indices are checked by the kernel, independently of source specialization.
use locus::kernel::{
    Context, Definitions, Mode, Proof, Term, Type, VarId, check_proof, check_type, infer_term,
};
use std::rc::Rc;
fn at(base: Type, claim: Term) -> Type {
    Type::Instance(Box::new(base), vec![claim].into())
}
fn value(raw: Term, claim: Term) -> Term {
    Term::Instance(Box::new(raw), vec![claim])
}

#[test]
#[doc = "spec: 2.28:5"]
fn family_arguments_and_constructor_payload_are_independently_checked() {
    let (mut defs, p) = Definitions::with_prelude();
    let e = defs
        .declare_enum_family(
            &[Type::Prop],
            &[
                Type::Tuple(vec![]),
                Type::Tuple(vec![Type::proof(Term::Bound(0))]),
            ],
        )
        .unwrap();
    let mut ctx = Context::with_definitions(Rc::new(defs));
    let truth = p.truth_prop();
    let falsehood = p.falsehood_prop();
    let proof = Term::proof(Proof::Construct {
        prop: p.truth,
        variant: 0,
        params: vec![],
        payload: vec![],
    });
    let some = value(Term::Variant(e, 1, vec![proof.clone()]), truth.clone());
    assert_eq!(
        infer_term(&mut ctx, &some, Mode::Executable),
        Ok(at(Type::Enum(e), truth.clone()))
    );
    assert!(
        infer_term(
            &mut ctx,
            &value(Term::Variant(e, 1, vec![proof]), falsehood.clone()),
            Mode::Executable
        )
        .is_err()
    );
    assert!(infer_term(&mut ctx, &Term::Variant(e, 0, vec![]), Mode::Logical).is_err());
    assert!(check_type(&mut ctx, &Type::Enum(e)).is_err());
    assert!(check_type(&mut ctx, &at(Type::Enum(e), Term::U8(0))).is_err());
    assert!(check_type(&mut ctx, &at(Type::Enum(e), Term::Free(VarId::fresh()))).is_err());
    // An instance is a constructor, never a cast that can retag an existing value.
    assert!(infer_term(&mut ctx, &value(some.clone(), falsehood), Mode::Logical).is_err());
    assert!(
        check_type(
            &mut ctx,
            &Type::Instance(Box::new(Type::Enum(e)), vec![].into())
        )
        .is_err()
    );
    assert!(
        check_type(
            &mut ctx,
            &Type::Instance(Box::new(Type::Enum(e)), vec![truth.clone(), truth].into())
        )
        .is_err()
    );
}
#[test]
#[doc = "spec: 2.28:5"]
fn parameter_and_field_binders_are_separate_telescopes() {
    let (mut defs, _) = Definitions::with_prelude();
    let param = VarId::fresh();
    let x = VarId::fresh();
    let claim = Term::eq(Type::U8, Term::Free(param), Term::Free(x));
    let fields = Type::tuple_over(&[(x, Type::U8), (VarId::fresh(), Type::proof(claim))]);
    let Type::Fn(parameters, fields) = Type::function_over(&[(param, Type::U8)], &fields) else {
        panic!()
    };
    let s = defs.declare_struct_family(&parameters, &fields).unwrap();
    let mut ctx = Context::with_definitions(Rc::new(defs));
    let raw = Term::Struct(s, vec![Term::U8(3), Term::proof(Proof::Refl(Term::U8(3)))]);
    let v = value(raw.clone(), Term::U8(3));
    assert_eq!(
        infer_term(&mut ctx, &v, Mode::Logical),
        Ok(at(Type::Struct(s), Term::U8(3)))
    );
    assert_eq!(
        infer_term(&mut ctx, &Term::proj(v.clone(), 1), Mode::Logical),
        Ok(Type::proof(Term::eq(
            Type::U8,
            Term::U8(3),
            Term::proj(v.clone(), 0)
        )))
    );
    assert!(infer_term(&mut ctx, &value(raw, Term::U8(4)), Mode::Logical).is_err());
    let projection = Term::proj(v, 0);
    assert!(
        check_proof(
            &mut ctx,
            &Proof::Projection(projection.clone()),
            &Term::eq(Type::U8, projection, Term::U8(3))
        )
        .is_ok()
    );
}
#[test]
fn malformed_families_are_rejected_even_when_no_variants_exist() {
    let (mut defs, _) = Definitions::with_prelude();
    let bad = Type::proof(Term::Free(VarId::fresh()));
    assert!(
        defs.declare_enum_family(std::slice::from_ref(&bad), &[])
            .is_err()
    );
    assert!(
        defs.declare_struct_family(&[bad], &Type::Tuple(vec![]))
            .is_err()
    );
    assert!(
        defs.declare_enum_family(
            &[Type::Prop],
            &[Type::Tuple(vec![Type::proof(Term::Bound(1))])]
        )
        .is_err()
    );
}
#[test]
fn indexed_certificates_round_trip_and_recheck() {
    use locus::store::text::{Names, parse_proof, parse_type, print_proof, print_type};
    let (mut defs, p) = Definitions::with_prelude();
    let s = defs
        .declare_struct_family(
            &[Type::Prop],
            &Type::Tuple(vec![Type::U8, Type::proof(Term::Bound(1))]),
        )
        .unwrap();
    let mut ctx = Context::with_definitions(Rc::new(defs));
    let mut names = Names::new();
    names.structure("Carried", s);
    names.proposition("True", p.truth);
    let ty = at(Type::Struct(s), p.truth_prop());
    let text = print_type(&ty, &ctx, &names).unwrap();
    assert_eq!(parse_type(&text, &ctx, &names).unwrap(), ty);
    let v = value(
        Term::Struct(
            s,
            vec![
                Term::U8(7),
                Term::proof(Proof::Construct {
                    prop: p.truth,
                    variant: 0,
                    params: vec![],
                    payload: vec![],
                }),
            ],
        ),
        p.truth_prop(),
    );
    let target = Term::proj(v, 0);
    let certificate = Proof::Projection(target.clone());
    let claim = Term::eq(Type::U8, target, Term::U8(7));
    let encoded = print_proof(&certificate, &ctx, &names).unwrap();
    let decoded = parse_proof(&encoded, &ctx, &names).unwrap();
    assert!(check_proof(&mut ctx, &decoded, &claim).is_ok());
    let altered = encoded.replace('7', "8");
    let corrupted = parse_proof(&altered, &ctx, &names).unwrap();
    assert!(check_proof(&mut ctx, &corrupted, &claim).is_err());
}
#[test]
fn family_indices_participate_in_bounded_traversal_and_substitution() {
    let (mut defs, p) = Definitions::with_prelude();
    let e = defs
        .declare_enum_family(&[Type::Prop], &[Type::Tuple(vec![])])
        .unwrap();
    let mut ctx = Context::with_definitions(Rc::new(defs));
    let n = VarId::fresh();
    let ty = at(Type::Enum(e), Term::Free(n));
    assert_eq!(
        ty.replace_var(n, &p.truth_prop()),
        at(Type::Enum(e), p.truth_prop())
    );
    let mut deep = p.truth_prop();
    for _ in 0..300 {
        deep = Term::implies(p.truth_prop(), deep);
    }
    assert!(check_type(&mut ctx, &at(Type::Enum(e), deep)).is_err());
    assert!(check_type(&mut ctx, &Type::Enum(e)).is_err());
}
#[test]
#[doc = "spec: 3.2:3, 2.28:6"]
fn execution_checker_recovers_only_the_instantiated_claim() {
    use locus::exec::{Arm, Block, ExecFn, Program, Promises, Stmt, Tail};
    use locus::kernel::HypId;
    let (mut defs, p) = Definitions::with_prelude();
    let e = defs
        .declare_enum_family(
            &[Type::Prop],
            &[
                Type::Tuple(vec![]),
                Type::Tuple(vec![Type::proof(Term::Bound(0))]),
            ],
        )
        .unwrap();
    for wrong in [false, true] {
        let claim = VarId::fresh();
        let opt = VarId::fresh();
        let fallback = VarId::fresh();
        let result = VarId::fresh();
        let h = VarId::fresh();
        let result_claim = if wrong {
            p.falsehood_prop()
        } else {
            Term::Free(claim)
        };
        let params = vec![
            (claim, Type::Prop),
            (opt, at(Type::Enum(e), Term::Free(claim))),
            (fallback, Type::proof(Term::Free(claim))),
        ];
        let returned = |v| Block {
            stmts: vec![],
            tail: Tail::Value(Term::proof(Proof::OfTerm(Term::Free(v)))),
        };
        let f = ExecFn {
            promises: Promises::default(),
            signature: Type::function_over(&params, &Type::proof(result_claim.clone())),
            params: vec![claim, opt, fallback],
            body: Block {
                stmts: vec![Stmt::Match {
                    var: result,
                    ty: Type::proof(result_claim),
                    scrutinee: Term::Free(opt),
                    arms: vec![
                        Arm {
                            payload: vec![],
                            fact: HypId::fresh(),
                            body: returned(fallback),
                        },
                        Arm {
                            payload: vec![h],
                            fact: HypId::fresh(),
                            body: returned(h),
                        },
                    ],
                }],
                tail: Tail::Value(Term::proof(Proof::OfTerm(Term::Free(result)))),
            },
        };
        assert_eq!(Program::new(defs.clone()).declare(f).is_ok(), !wrong);
    }
}

#[test]
#[doc = "spec: 2.21:1, 2.28:5"]
fn family_parameter_depth_is_checked_before_cloning() {
    use locus::kernel::KernelError;
    let mut deep = Type::U8;
    for _ in 0..100_000 {
        deep = Type::Boxed(Box::new(deep));
    }
    let mut definitions = Definitions::new();
    let params = std::slice::from_ref(&deep);
    assert_eq!(
        definitions.declare_enum_family(params, &[]),
        Err(KernelError::TooDeep)
    );
    assert_eq!(
        definitions.declare_struct_family(params, &Type::Tuple(vec![])),
        Err(KernelError::TooDeep)
    );
    // Dropping this deliberately hostile input would itself recurse in Rust.
    std::mem::forget(deep);
}

#[test]
#[doc = "spec: 2.28:6"]
fn logical_indices_cannot_hide_negative_recursive_witnesses() {
    use locus::kernel::{KernelError, PropVariant};
    let (mut defs, prelude) = Definitions::with_prelude();
    let carrier = defs
        .declare_struct_family(
            &[Type::Prop],
            &Type::Tuple(vec![Type::proof(Term::Bound(0))]),
        )
        .unwrap();
    let mut reserved = None;
    let rejected = defs.declare_inductive_prop(vec![], |recursive| {
        reserved = Some(recursive);
        let negative = Term::implies(Term::PropApp(recursive, vec![]), prelude.falsehood_prop());
        vec![PropVariant::arm(
            Type::Tuple(vec![at(Type::Struct(carrier), negative)]),
            |_| prelude.truth_prop(),
        )]
    });
    assert_eq!(
        rejected,
        Err(KernelError::InvalidRecursion(
            "recursive predicates cannot occur in witness types"
        ))
    );
    // The rejected declaration must not leave its provisional predicate behind.
    let accepted = defs
        .declare_inductive_prop(vec![], |_| {
            vec![PropVariant::arm(Type::Tuple(vec![]), |_| {
                prelude.truth_prop()
            })]
        })
        .unwrap();
    assert_eq!(Some(accepted), reserved);
}
