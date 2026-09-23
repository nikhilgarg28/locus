//! Library quantifiers use only ordinary proposition cases and logical callables.
use locus::kernel::{
    Context, Definitions, Mode, Proof, Quantifiers, Term, Type, VarId, check_proof, infer_term,
};
use std::rc::Rc;

fn reflexive_at(predicate: &Term, value: &Term) -> Proof {
    let call = Term::call(predicate.clone(), vec![value.clone()]);
    let reversed = Proof::transport(
        Proof::Definition(call.clone()),
        |body| Term::eq(Type::Prop, body, call.clone()),
        Proof::Refl(call.clone()),
    );
    Proof::transport(reversed, |claim| claim, Proof::Refl(value.clone()))
}

#[test]
#[doc = "spec: 2.34:1, 2.34:2"]
fn library_quantifiers_construct_and_eliminate_without_native_quantifier_nodes() {
    let (mut defs, _) = Definitions::with_prelude();
    let q = Quantifiers::declare(&mut defs, Type::Int).unwrap();
    let x = VarId::fresh();
    let p = Term::lambda_over(
        &[(x, Type::Int)],
        &Type::Prop,
        Term::eq(Type::Int, Term::Free(x), Term::Free(x)),
    );
    let result = Type::proof(Term::call(p.clone(), vec![Term::Free(x)]));
    let each = Term::lambda_over(
        &[(x, Type::Int)],
        &result,
        Term::proof(reflexive_at(&p, &Term::Free(x))),
    );
    let universal = q.each(&defs, p.clone(), each).unwrap();
    let mut ctx = Context::with_definitions(Rc::new(defs.clone()));
    assert_eq!(
        check_proof(&mut ctx, &universal, &q.forall(p.clone())),
        Ok(())
    );
    let five = Term::int(5);
    let instance = q.specialize(universal, p.clone(), five.clone());
    let at_five = Term::call(p.clone(), vec![five.clone()]);
    assert_eq!(check_proof(&mut ctx, &instance, &at_five), Ok(()));
    assert!(
        check_proof(
            &mut ctx,
            &instance,
            &Term::eq(Type::Int, five.clone(), Term::int(6))
        )
        .is_err()
    );
    let witness = q.witness(p.clone(), five.clone(), reflexive_at(&p, &five));
    assert_eq!(
        check_proof(&mut ctx, &witness, &q.exists(p.clone())),
        Ok(())
    );
    // A witness is usable while deriving another proof, including repackaging it.
    let rebuilt = q.eliminate(witness.clone(), q.exists(p.clone()), |value, evidence| {
        q.witness(p.clone(), value, evidence)
    });
    assert_eq!(
        check_proof(&mut ctx, &rebuilt, &q.exists(p.clone())),
        Ok(())
    );
    // Term-level matching on evidence cannot expose its witness as Int data.
    let extract = Term::case(
        Term::proof(witness),
        Type::Int,
        vec![(2, Box::new(|v, _| v[0].clone()))],
    );
    assert!(infer_term(&mut ctx, &extract, Mode::Logical).is_err());
    assert!(Quantifiers::from_declarations(&defs, Type::Int, q.exists_id(), q.forall_id()).is_ok());
    assert!(
        Quantifiers::from_declarations(&defs, Type::Int, q.forall_id(), q.exists_id()).is_err()
    );
}

#[test]
#[doc = "spec: 2.39:1"]
fn only_registered_quantifier_schemas_extend_positive_predicate_induction() {
    use locus::kernel::{PropVariant, TermArm};
    let (mut defs, prelude) = Definitions::with_prelude();
    let q = Quantifiers::declare(&mut defs, Type::Int).unwrap();
    let witness_fields = Type::Tuple(vec![Type::Int]);
    let body = |recursive, outer: Term, negated: bool| {
        let x = VarId::fresh();
        let instance = Term::PropApp(recursive, vec![outer]);
        let instance = if negated {
            prelude.not_prop(instance)
        } else {
            instance
        };
        q.exists(Term::lambda_over(&[(x, Type::Int)], &Type::Prop, instance))
    };
    // A declaration's name or shape alone grants no positivity privilege.
    assert!(
        defs.declare_inductive_prop(vec![Type::Int], |recursive| vec![PropVariant::arm(
            witness_fields.clone(),
            |v| body(recursive, v[0].clone(), false)
        )])
        .is_err()
    );
    defs.register_quantifiers(Type::Int, q.exists_id(), q.forall_id())
        .unwrap();
    assert!(
        defs.declare_inductive_prop(vec![Type::Int], |recursive| vec![PropVariant::arm(
            witness_fields.clone(),
            |v| body(recursive, v[0].clone(), true)
        )])
        .is_err()
    );
    let p = defs
        .declare_inductive_prop(vec![Type::Int], |recursive| {
            vec![
                PropVariant::arm(witness_fields.clone(), |_| prelude.truth_prop()),
                PropVariant::arm(witness_fields.clone(), |v| {
                    body(recursive, v[0].clone(), false)
                }),
            ]
        })
        .unwrap();
    let mut ctx = Context::with_definitions(Rc::new(defs));
    let target = Term::PropApp(p, vec![Term::int(4)]);
    let h = ctx.assume(target).unwrap();
    let truth = || Proof::Construct {
        prop: prelude.truth,
        variant: 0,
        params: vec![],
        payload: vec![],
    };
    let proof = Proof::PropInduction {
        scrutinee: Box::new(Proof::hyp(h)),
        motive: TermArm {
            binders: 1,
            body: prelude.truth_prop(),
        },
        arms: vec![
            Proof::arm(2, 1, |_, _| truth()),
            Proof::arm(2, 1, |_, _| truth()),
        ],
    };
    assert_eq!(check_proof(&mut ctx, &proof, &prelude.truth_prop()), Ok(()));
    assert!(check_proof(&mut ctx, &proof, &prelude.falsehood_prop()).is_err());
}
