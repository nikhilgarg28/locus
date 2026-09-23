//! D1: generic templates expand to ordinary checked declarations.
use locus::kernel::{
    Context, Definitions, GenericDeclaration, GenericInstance, KernelError, Mode, Proof,
    PropVariant, Term, Type, TypeBound, check_proof, check_type, infer_term,
};
use std::rc::Rc;

#[test]
#[doc = "spec: 2.28:1, 2.28:2, 2.28:4"]
fn each_declaration_kind_instantiates_at_two_types_and_is_cached() {
    let (mut defs, _) = Definitions::with_prelude();
    let structure = defs
        .declare_generic(vec![TypeBound::Any], |t| {
            GenericDeclaration::Struct(Type::Tuple(vec![t[0].clone()]))
        })
        .unwrap();
    let enumeration = defs
        .declare_generic(vec![TypeBound::Any], |t| {
            GenericDeclaration::Enum(vec![Type::Tuple(vec![]), Type::Tuple(vec![t[0].clone()])])
        })
        .unwrap();
    let function = defs
        .declare_generic(vec![TypeBound::Any], |t| {
            GenericDeclaration::function(
                Type::Fn(vec![t[0].clone()], Box::new(t[0].clone())),
                |p| p[0].clone(),
            )
        })
        .unwrap();
    let proposition = defs
        .declare_generic(vec![TypeBound::Any], |t| GenericDeclaration::Proposition {
            params: vec![t[0].clone()],
            variants: vec![PropVariant::arm(Type::Tuple(vec![t[0].clone()]), |p| {
                Term::eq(t[0].clone(), p[0].clone(), p[0].clone())
            })],
        })
        .unwrap();
    let mut previous = Vec::new();
    for (ty, value) in [(Type::Int, Term::int(3)), (Type::U8, Term::U8(3))] {
        let s = defs
            .instantiate_generic(structure, std::slice::from_ref(&ty))
            .unwrap();
        let e = defs
            .instantiate_generic(enumeration, std::slice::from_ref(&ty))
            .unwrap();
        let f = defs
            .instantiate_generic(function, std::slice::from_ref(&ty))
            .unwrap();
        let p = defs
            .instantiate_generic(proposition, std::slice::from_ref(&ty))
            .unwrap();
        for (generic, instance) in [
            (structure, s),
            (enumeration, e),
            (function, f),
            (proposition, p),
        ] {
            assert_eq!(
                defs.instantiate_generic(generic, std::slice::from_ref(&ty)),
                Ok(instance)
            );
            assert!(!previous.contains(&instance));
            previous.push(instance);
        }
        let GenericInstance::Struct(s) = s else {
            panic!()
        };
        let GenericInstance::Enum(e) = e else {
            panic!()
        };
        let GenericInstance::Function(f) = f else {
            panic!()
        };
        let GenericInstance::Proposition(p) = p else {
            panic!()
        };
        let mut ctx = Context::with_definitions(Rc::new(defs.clone()));
        assert_eq!(
            infer_term(
                &mut ctx,
                &Term::Struct(s, vec![value.clone()]),
                Mode::Logical
            ),
            Ok(Type::Struct(s))
        );
        assert_eq!(
            infer_term(
                &mut ctx,
                &Term::Variant(e, 1, vec![value.clone()]),
                Mode::Logical
            ),
            Ok(Type::Enum(e))
        );
        assert_eq!(
            infer_term(
                &mut ctx,
                &Term::call(Term::Fn(f), vec![value.clone()]),
                Mode::Logical
            ),
            Ok(ty)
        );
        assert_eq!(
            check_proof(
                &mut ctx,
                &Proof::Construct {
                    prop: p,
                    variant: 0,
                    params: vec![value.clone()],
                    payload: vec![Term::proof(Proof::Refl(value.clone()))]
                },
                &Term::PropApp(p, vec![value])
            ),
            Ok(())
        );
    }
}

#[test]
#[doc = "spec: 2.28:3"]
fn logical_bounds_and_derived_aggregate_classification_are_checked() {
    let (mut defs, _) = Definitions::with_prelude();
    // D4 adds recursive tails; D1 establishes bounded element instances.
    let seq = defs
        .declare_generic(vec![TypeBound::Logical], |t| {
            GenericDeclaration::Enum(vec![Type::Tuple(vec![]), Type::Tuple(vec![t[0].clone()])])
        })
        .unwrap();
    assert!(defs.instantiate_generic(seq, &[Type::Int]).is_ok());
    assert_eq!(
        defs.instantiate_generic(seq, &[Type::U8]),
        Err(KernelError::NotLogicalType(Type::U8))
    );
    assert!(defs.instantiate_generic(seq, &[]).is_err());
    let good = defs
        .declare_struct(&Type::Tuple(vec![Type::Int, Type::Bool]))
        .unwrap();
    let bad = defs.declare_struct(&Type::Tuple(vec![Type::U8])).unwrap();
    assert!(!defs.is_logical_type(&Type::Struct(good)));
    defs.mark_logical(&Type::Struct(good)).unwrap();
    assert!(defs.is_logical_type(&Type::Struct(good)));
    assert!(defs.instantiate_generic(seq, &[Type::Struct(good)]).is_ok());
    assert_eq!(
        defs.mark_logical(&Type::Struct(bad)),
        Err(KernelError::NotLogicalType(Type::U8))
    );
    assert!(!defs.is_logical_type(&Type::Struct(bad)));
}

#[test]
fn templates_cannot_introduce_unchecked_proofs_or_leak_parameter_types() {
    let (mut defs, _) = Definitions::with_prelude();
    let mut escaped = Type::U8;
    let generic = defs
        .declare_generic(vec![TypeBound::Any], |t| {
            escaped = t[0].clone();
            GenericDeclaration::function(
                Type::Fn(
                    vec![t[0].clone()],
                    Box::new(Type::proof(Term::eq(
                        t[0].clone(),
                        Term::Bound(0),
                        Term::Bound(0),
                    ))),
                ),
                |p| Term::proof(Proof::Refl(p[0].clone())),
            )
        })
        .unwrap();
    let mut ctx = Context::with_definitions(Rc::new(defs.clone()));
    assert!(check_type(&mut ctx, &escaped).is_err());
    let malformed = defs
        .declare_generic(vec![TypeBound::Any], |_| {
            GenericDeclaration::Struct(Type::Tuple(vec![escaped.clone()]))
        })
        .unwrap();
    assert_eq!(
        defs.instantiate_generic(malformed, &[Type::U8]),
        Err(KernelError::UnknownStruct)
    );
    assert!(defs.instantiate_generic(generic, &[escaped]).is_err());
    let forged = defs
        .declare_generic(vec![TypeBound::Any], |t| {
            GenericDeclaration::function(
                Type::Fn(
                    vec![t[0].clone()],
                    Box::new(Type::proof(Term::eq(Type::U8, Term::U8(0), Term::U8(1)))),
                ),
                |_| Term::proof(Proof::Omitted),
            )
        })
        .unwrap();
    assert_eq!(
        defs.instantiate_generic(forged, &[Type::U8]),
        Err(KernelError::OmittedProof)
    );
    let GenericInstance::Function(f) = defs.instantiate_generic(generic, &[Type::Int]).unwrap()
    else {
        panic!()
    };
    let mut ctx = Context::with_definitions(Rc::new(defs));
    let term = Term::call(Term::Fn(f), vec![Term::int(8)]);
    let expected = Type::proof(Term::eq(Type::Int, Term::int(8), Term::int(8)));
    assert_eq!(infer_term(&mut ctx, &term, Mode::Logical), Ok(expected));
}

#[test]
fn logical_restriction_removes_runtime_permission_for_boolean_bodies() {
    let mut defs = Definitions::new();
    let f = defs
        .declare_fn(&Type::Fn(vec![], Box::new(Type::Bool)), |_| {
            Term::Bool(true)
        })
        .unwrap();
    assert!(defs.is_executable(f));
    defs.restrict_to_logic(f).unwrap();
    assert!(!defs.is_executable(f));
    let mut ctx = Context::with_definitions(Rc::new(defs));
    assert_eq!(
        infer_term(&mut ctx, &Term::Fn(f), Mode::Executable),
        Err(KernelError::LogicalFunctionInExecutable)
    );
    assert!(infer_term(&mut ctx, &Term::call(Term::Fn(f), vec![]), Mode::Executable).is_err());
}

#[test]
fn logical_nominal_tags_and_fields_cannot_leak_into_execution() {
    let mut defs = Definitions::new();
    let logical = defs
        .declare_enum(&[Type::Tuple(vec![]), Type::Tuple(vec![])])
        .unwrap();
    defs.mark_logical(&Type::Enum(logical)).unwrap();
    let record = defs.declare_struct(&Type::Tuple(vec![Type::Bool])).unwrap();
    defs.mark_logical(&Type::Struct(record)).unwrap();
    let runtime = defs
        .declare_enum(&[Type::Tuple(vec![]), Type::Tuple(vec![Type::Enum(logical)])])
        .unwrap();
    let mut ctx = Context::with_definitions(Rc::new(defs));
    let hidden = Term::Variant(logical, 0, vec![]);
    assert!(infer_term(&mut ctx, &hidden, Mode::Executable).is_err());
    let variable = Term::var(ctx.declare(Type::Enum(logical)).unwrap());
    assert!(infer_term(&mut ctx, &variable, Mode::Executable).is_err());
    let field = Term::proj(Term::Struct(record, vec![Term::Bool(true)]), 0);
    assert!(infer_term(&mut ctx, &field, Mode::Executable).is_err());
    // An ordinary enum with an erased field still has an executable tag.
    let contained = Term::Variant(runtime, 1, vec![hidden]);
    assert_eq!(
        infer_term(&mut ctx, &contained, Mode::Executable),
        Ok(Type::Enum(runtime))
    );
}
