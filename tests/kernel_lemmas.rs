//! K8: the runtime comparisons at every machine type, the axiom schema
//! `cmp_reflect` that connects each to the order of the views, and the
//! lemmas of `src/kernel/theory.rs` about `Int` and about every machine
//! type, each used once in a proof the kernel checks, at variables and at
//! literals. The names of the lemmas are listed in full, so that a rename
//! is deliberate: E5 reads the same table to expose them to source.

use std::rc::Rc;
use std::time::Instant;

use locus::kernel::theory::{self, MachineLemmas, Theory};
use locus::kernel::{
    Axiom, CmpOp, Context, Definitions, HypId, Integer, KernelError, MachineInt, Mode, Prelude,
    Prim, Proof, Term, Type, check_proof, evaluate_primitive, infer_proof, infer_term,
};

use MachineInt::{I8, I16, I32, I64, U8, U16, U32};

fn setup() -> (Rc<Definitions>, Prelude, Theory) {
    let (mut definitions, prelude) = Definitions::with_prelude();
    let theory = theory::declare(&mut definitions, &prelude).expect("the theory checks");
    (Rc::new(definitions), prelude, theory)
}

fn lit(ty: MachineInt, value: i128) -> Term {
    Term::machine(ty, Integer::from(value))
}

fn view(ty: MachineInt, x: &Term) -> Term {
    Term::view(ty, x.clone())
}

fn cmp(op: CmpOp, ty: MachineInt, a: &Term, b: &Term) -> Term {
    Term::cmp(op, ty, a.clone(), b.clone())
}

fn is(c: &Term, flag: bool) -> Term {
    Term::eq(Type::Bool, c.clone(), Term::Bool(flag))
}

fn int_eq(a: &Term, b: &Term) -> Term {
    Term::eq(Type::Int, a.clone(), b.clone())
}

fn le(a: &Term, b: &Term) -> Term {
    Term::int_le(a.clone(), b.clone())
}

fn lt(a: &Term, b: &Term) -> Term {
    Term::int_lt(a.clone(), b.clone())
}

fn call(id: locus::kernel::FnId, arguments: Vec<Term>) -> Proof {
    Proof::OfTerm(Term::call(Term::Fn(id), arguments))
}

/// A context with variables and hypotheses declared as a lemma's use needs.
struct Scene {
    ctx: Context,
    prelude: Prelude,
}

impl Scene {
    fn new(definitions: &Rc<Definitions>, prelude: Prelude) -> Self {
        Self {
            ctx: Context::with_definitions(Rc::clone(definitions)),
            prelude,
        }
    }

    fn var(&mut self, ty: Type) -> Term {
        Term::var(self.ctx.declare(ty).expect("a variable"))
    }

    /// A hypothesis, as the proof term a lemma call takes.
    fn assume(&mut self, prop: Term) -> Term {
        Term::proof(self.assume_proof(prop))
    }

    /// A hypothesis, as a proof.
    fn assume_proof(&mut self, prop: Term) -> Proof {
        let id: HypId = self.ctx.assume(prop).expect("a hypothesis");
        Proof::hyp(id)
    }

    fn not(&self, prop: Term) -> Term {
        self.prelude.not_prop(prop)
    }

    fn check(&mut self, proof: &Proof, claim: &Term) {
        if let Err(error) = check_proof(&mut self.ctx, proof, claim) {
            panic!("{error}\n  claim: {claim}");
        }
    }

    fn infer(&mut self, proof: &Proof) -> Result<Term, KernelError> {
        infer_proof(&mut self.ctx, proof)
    }
}

/// The boundary set of a type: the ends of its range, zero, and their
/// neighbours, within the range.
fn boundary(ty: MachineInt) -> Vec<i128> {
    let (min, max) = (
        ty.min().to_i128().expect("fits"),
        ty.max().to_i128().expect("fits"),
    );
    let mut out: Vec<i128> = vec![min, min + 1, min + 2, -1, 0, 1, 2, max - 2, max - 1, max];
    out.retain(|value| min <= *value && *value <= max);
    out.sort_unstable();
    out.dedup();
    out
}

/// The number of proof nodes, counting the proofs embedded as arguments of
/// a lemma call or as the payload of a constructor.
fn nodes(proof: &Proof) -> usize {
    let embedded = |terms: &[Term]| -> usize {
        terms
            .iter()
            .map(|term| match term {
                Term::Proof(proof) => nodes(proof),
                _ => 0,
            })
            .sum()
    };
    1 + match proof {
        Proof::OfTerm(Term::Call(_, arguments)) => embedded(arguments),
        Proof::Transport { eq, proof, .. } => nodes(eq) + nodes(proof),
        Proof::ImpliesIntro { body, .. } | Proof::ForallIntro { body, .. } => nodes(body),
        Proof::ImpliesElim(left, right) => nodes(left) + nodes(right),
        Proof::ForallElim(universal, _) => nodes(universal),
        Proof::Construct { payload, .. } => embedded(payload),
        Proof::CaseProof {
            scrutinee, arms, ..
        } => nodes(scrutinee) + arms.iter().map(|arm| nodes(&arm.body)).sum::<usize>(),
        Proof::CaseData { arms, .. } => arms.iter().map(|arm| nodes(&arm.body)).sum(),
        Proof::ExistsIntro { proof, .. } => nodes(proof),
        Proof::ExistsElim { exists, arm, .. } => nodes(exists) + nodes(&arm.body),
        Proof::Linear { pairs, .. } => pairs.iter().map(|(proof, _)| nodes(proof)).sum(),
        _ => 0,
    }
}

// --- The names -----------------------------------------------------------------------

/// The lemmas about one machine type, in the order of the table.
const MACHINE_LEMMAS: [&str; 18] = [
    "le_refl",
    "le_trans",
    "le_of_lt",
    "lt_of_le_of_ne",
    "lt_irrefl",
    "le_antisymm",
    "view_injective",
    "view_bounds",
    "le_of_cmp",
    "cmp_of_le",
    "lt_of_cmp",
    "cmp_of_lt",
    "eq_of_cmp",
    "cmp_of_eq",
    "lt_of_not_le",
    "le_of_not_lt",
    "succ_le_of_lt",
    "eq_symm",
];

/// The three more at each unsigned type.
const UNSIGNED_LEMMAS: [&str; 3] = ["zero_le", "sub_le", "sub_le_sub"];

const INT_LEMMAS: [&str; 6] = [
    "int_le_of_lt",
    "int_lt_of_le_of_ne",
    "int_le_add_left",
    "int_le_add_right",
    "int_le_sub",
    "int_mul_le_mul_nonneg",
];

/// The names E5 exposes to source, exactly, in the order the table gives
/// them. A change here is a rename the language sees.
#[test]
#[doc = "spec: 2.14:9"]
fn the_lemma_names_are_stable() {
    let (definitions, _, theory) = setup();
    let mut expected: Vec<String> = INT_LEMMAS.iter().map(|name| name.to_string()).collect();
    for ty in MachineInt::ALL {
        for lemma in MACHINE_LEMMAS {
            expected.push(format!("{}_{lemma}", ty.kernel_name()));
        }
        if !ty.signed() {
            for lemma in UNSIGNED_LEMMAS {
                expected.push(format!("{}_{lemma}", ty.kernel_name()));
            }
        }
    }
    let names = theory.lemma_names();
    let found: Vec<String> = names.iter().map(|(name, _)| name.to_string()).collect();
    assert_eq!(found, expected);
    assert_eq!(names.len(), 6 + 12 * 18 + 6 * 3);

    // Distinct names, distinct identities, each a declared function.
    let mut distinct = found.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), found.len());
    let mut ids: Vec<usize> = names
        .iter()
        .map(|(name, id)| {
            assert!(definitions.signature(*id).is_some(), "{name} is declared");
            format!("{id:?}")
        })
        .map(|text| {
            text.trim_start_matches("FnId(")
                .trim_end_matches(')')
                .parse()
                .expect("an index")
        })
        .collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), names.len(), "no two names share a lemma");

    // The lookup by type agrees with the table.
    for ty in MachineInt::ALL {
        let family = theory.machine(ty);
        assert_eq!(
            family.names(ty)[0],
            (
                names
                    .iter()
                    .find(|(name, _)| *name == format!("{}_le_refl", ty.kernel_name()))
                    .expect("in the table")
                    .0,
                family.le_refl
            )
        );
    }

    // The names the examples call are the family at `u8`: E5 rebound them
    // from the lemmas over the earlier model of `u8` to the table.
    let rebound = [
        ("u8_le_refl", theory.machine(MachineInt::U8).le_refl),
        ("u8_le_trans", theory.machine(MachineInt::U8).le_trans),
        (
            "u8_lt_of_le_of_ne",
            theory.machine(MachineInt::U8).lt_of_le_of_ne,
        ),
        (
            "u8_succ_le_of_lt",
            theory.machine(MachineInt::U8).succ_le_of_lt,
        ),
        ("u8_eq_symm", theory.machine(MachineInt::U8).eq_symm),
        (
            "u8_zero_le",
            theory.machine(MachineInt::U8).unsigned.unwrap().zero_le,
        ),
        (
            "u8_sub_le",
            theory.machine(MachineInt::U8).unsigned.unwrap().sub_le,
        ),
        (
            "u8_sub_le_sub",
            theory.machine(MachineInt::U8).unsigned.unwrap().sub_le_sub,
        ),
    ];
    for (name, id) in rebound {
        let (_, in_table) = names
            .iter()
            .find(|(other, _)| *other == name)
            .expect("the family has the name");
        assert_eq!(*in_table, id, "{name} in the table is the lemma over views");
    }
    // A signed type has no `zero_le`.
    assert!(!names.iter().any(|(name, _)| *name == "i8_zero_le"));
}

// --- Every lemma at every type, at variables and at literals ----------------------------

/// Uses every lemma of the family at `ty` once, at variables whose premises
/// are hypotheses, and checks each conclusion is what the table says.
fn use_machine_family_at_variables(
    definitions: &Rc<Definitions>,
    prelude: Prelude,
    ty: MachineInt,
    family: &MachineLemmas,
) {
    let over = Type::machine(ty);
    let mut s = Scene::new(definitions, prelude);
    let (a, b, c) = (
        s.var(over.clone()),
        s.var(over.clone()),
        s.var(over.clone()),
    );
    let (va, vb, vc) = (view(ty, &a), view(ty, &b), view(ty, &c));
    let same = |x: &Term, y: &Term| Term::eq(over.clone(), x.clone(), y.clone());

    s.check(&call(family.le_refl, vec![a.clone()]), &le(&va, &va));

    let h_ab = s.assume(le(&va, &vb));
    let h_bc = s.assume(le(&vb, &vc));
    s.check(
        &call(
            family.le_trans,
            vec![a.clone(), b.clone(), c.clone(), h_ab.clone(), h_bc],
        ),
        &le(&va, &vc),
    );

    let strict = s.assume(lt(&va, &vb));
    s.check(
        &call(family.le_of_lt, vec![a.clone(), b.clone(), strict]),
        &le(&va, &vb),
    );

    let differ = s.assume(s.not(int_eq(&va, &vb)));
    s.check(
        &call(
            family.lt_of_le_of_ne,
            vec![a.clone(), b.clone(), h_ab.clone(), differ],
        ),
        &lt(&va, &vb),
    );

    s.check(
        &call(family.lt_irrefl, vec![a.clone()]),
        &s.not(lt(&va, &va)),
    );

    let h_ba = s.assume(le(&vb, &va));
    s.check(
        &call(family.le_antisymm, vec![a.clone(), b.clone(), h_ab, h_ba]),
        &same(&a, &b),
    );

    let views_equal = s.assume(int_eq(&va, &vb));
    s.check(
        &call(
            family.view_injective,
            vec![a.clone(), b.clone(), views_equal],
        ),
        &same(&a, &b),
    );

    s.check(
        &call(family.view_bounds, vec![a.clone()]),
        &prelude.and_prop(le(&Term::Int(ty.min()), &va), le(&va, &Term::Int(ty.max()))),
    );

    let (c_le, c_lt, c_eq) = (
        cmp(CmpOp::Le, ty, &a, &b),
        cmp(CmpOp::Lt, ty, &a, &b),
        cmp(CmpOp::Eq, ty, &a, &b),
    );
    let le_true = s.assume(is(&c_le, true));
    s.check(
        &call(family.le_of_cmp, vec![a.clone(), b.clone(), le_true]),
        &le(&va, &vb),
    );
    let ordered = s.assume(le(&va, &vb));
    s.check(
        &call(family.cmp_of_le, vec![a.clone(), b.clone(), ordered]),
        &is(&c_le, true),
    );
    let lt_true = s.assume(is(&c_lt, true));
    s.check(
        &call(family.lt_of_cmp, vec![a.clone(), b.clone(), lt_true]),
        &lt(&va, &vb),
    );
    let strict = s.assume(lt(&va, &vb));
    s.check(
        &call(family.cmp_of_lt, vec![a.clone(), b.clone(), strict]),
        &is(&c_lt, true),
    );
    let eq_true = s.assume(is(&c_eq, true));
    s.check(
        &call(family.eq_of_cmp, vec![a.clone(), b.clone(), eq_true]),
        &same(&a, &b),
    );
    let equal = s.assume(same(&a, &b));
    s.check(
        &call(family.cmp_of_eq, vec![a.clone(), b.clone(), equal]),
        &is(&c_eq, true),
    );
    let le_false = s.assume(is(&c_le, false));
    s.check(
        &call(family.lt_of_not_le, vec![a.clone(), b.clone(), le_false]),
        &lt(&vb, &va),
    );
    let lt_false = s.assume(is(&c_lt, false));
    s.check(
        &call(family.le_of_not_lt, vec![a.clone(), b.clone(), lt_false]),
        &le(&vb, &va),
    );
}

/// Uses every lemma of the family at `ty` once at literals, with each
/// premise decided by evaluation, and checks the conclusion.
fn use_machine_family_at_literals(
    definitions: &Rc<Definitions>,
    prelude: Prelude,
    ty: MachineInt,
    family: &MachineLemmas,
) {
    let over = Type::machine(ty);
    let mut s = Scene::new(definitions, prelude);
    // Three values in order, and the ends of the range.
    let (lo, hi) = (ty.min().to_i128().unwrap(), ty.max().to_i128().unwrap());
    let (a, b, c) = (lit(ty, lo + 1), lit(ty, lo + 2), lit(ty, hi));
    let (va, vb, vc) = (view(ty, &a), view(ty, &b), view(ty, &c));
    let same = |x: &Term, y: &Term| Term::eq(over.clone(), x.clone(), y.clone());
    // A closed comparison over `Int`, or a closed equation, decided by
    // evaluation; a closed comparison of `bool`, computed by evaluation.
    let decide = |prop: &Term| Term::proof(Proof::Evaluate(prop.clone()));
    let computed = |c: &Term| Term::proof(Proof::Evaluate(c.clone()));

    s.check(&call(family.le_refl, vec![a.clone()]), &le(&va, &va));
    s.check(
        &call(
            family.le_trans,
            vec![
                a.clone(),
                b.clone(),
                c.clone(),
                decide(&le(&va, &vb)),
                decide(&le(&vb, &vc)),
            ],
        ),
        &le(&va, &vc),
    );
    s.check(
        &call(
            family.le_of_lt,
            vec![a.clone(), b.clone(), decide(&lt(&va, &vb))],
        ),
        &le(&va, &vb),
    );
    // `evaluate` of a false equation proves its negation.
    s.check(
        &call(
            family.lt_of_le_of_ne,
            vec![
                a.clone(),
                b.clone(),
                decide(&le(&va, &vb)),
                decide(&int_eq(&va, &vb)),
            ],
        ),
        &lt(&va, &vb),
    );
    s.check(
        &call(family.lt_irrefl, vec![a.clone()]),
        &s.not(lt(&va, &va)),
    );
    s.check(
        &call(
            family.le_antisymm,
            vec![
                a.clone(),
                a.clone(),
                decide(&le(&va, &va)),
                decide(&le(&va, &va)),
            ],
        ),
        &same(&a, &a),
    );
    s.check(
        &call(
            family.view_injective,
            vec![b.clone(), b.clone(), decide(&int_eq(&vb, &vb))],
        ),
        &same(&b, &b),
    );
    s.check(
        &call(family.view_bounds, vec![c.clone()]),
        &prelude.and_prop(le(&Term::Int(ty.min()), &vc), le(&vc, &Term::Int(ty.max()))),
    );
    let (c_le, c_lt, c_eq) = (
        cmp(CmpOp::Le, ty, &a, &b),
        cmp(CmpOp::Lt, ty, &a, &b),
        cmp(CmpOp::Eq, ty, &b, &b),
    );
    s.check(
        &call(
            family.le_of_cmp,
            vec![a.clone(), b.clone(), computed(&c_le)],
        ),
        &le(&va, &vb),
    );
    s.check(
        &call(
            family.cmp_of_le,
            vec![a.clone(), b.clone(), decide(&le(&va, &vb))],
        ),
        &is(&c_le, true),
    );
    s.check(
        &call(
            family.lt_of_cmp,
            vec![a.clone(), b.clone(), computed(&c_lt)],
        ),
        &lt(&va, &vb),
    );
    s.check(
        &call(
            family.cmp_of_lt,
            vec![a.clone(), b.clone(), decide(&lt(&va, &vb))],
        ),
        &is(&c_lt, true),
    );
    s.check(
        &call(
            family.eq_of_cmp,
            vec![b.clone(), b.clone(), computed(&c_eq)],
        ),
        &same(&b, &b),
    );
    s.check(
        &call(
            family.cmp_of_eq,
            vec![b.clone(), b.clone(), Term::proof(Proof::Refl(b.clone()))],
        ),
        &is(&c_eq, true),
    );
    // The comparisons the other way round are false, and evaluation says so.
    let (le_ba, lt_ba) = (cmp(CmpOp::Le, ty, &c, &a), cmp(CmpOp::Lt, ty, &b, &a));
    s.check(
        &call(
            family.lt_of_not_le,
            vec![c.clone(), a.clone(), computed(&le_ba)],
        ),
        &lt(&va, &vc),
    );
    s.check(
        &call(
            family.le_of_not_lt,
            vec![b.clone(), a.clone(), computed(&lt_ba)],
        ),
        &le(&va, &vb),
    );
}

#[test]
#[doc = "spec: 2.12:11, 2.14:4, 2.14:5, 2.14:6, 2.14:7"]
fn every_machine_lemma_is_used_once_at_every_type() {
    let (definitions, prelude, theory) = setup();
    for ty in MachineInt::FIXED {
        let family = theory.machine(ty);
        use_machine_family_at_variables(&definitions, prelude, ty, family);
        use_machine_family_at_literals(&definitions, prelude, ty, family);
    }
}

#[test]
#[doc = "spec: 1.7:4, 2.14:1, 2.14:3"]
fn every_int_lemma_is_used_once() {
    let (definitions, prelude, theory) = setup();
    let mut s = Scene::new(&definitions, prelude);
    let (a, b, c) = (s.var(Type::Int), s.var(Type::Int), s.var(Type::Int));
    let add = |x: &Term, y: &Term| Term::int_add(x.clone(), y.clone());
    let sub = |x: &Term, y: &Term| Term::int_sub(x.clone(), y.clone());
    let mul = |x: &Term, y: &Term| Term::int_mul(x.clone(), y.clone());

    let strict = s.assume(lt(&a, &b));
    s.check(
        &call(theory.int_le_of_lt, vec![a.clone(), b.clone(), strict]),
        &le(&a, &b),
    );
    let h_ab = s.assume(le(&a, &b));
    let differ = s.assume(s.not(int_eq(&a, &b)));
    s.check(
        &call(
            theory.int_lt_of_le_of_ne,
            vec![a.clone(), b.clone(), h_ab.clone(), differ],
        ),
        &lt(&a, &b),
    );
    s.check(
        &call(
            theory.int_le_add_left,
            vec![a.clone(), b.clone(), c.clone(), h_ab.clone()],
        ),
        &le(&add(&c, &a), &add(&c, &b)),
    );
    s.check(
        &call(
            theory.int_le_add_right,
            vec![a.clone(), b.clone(), c.clone(), h_ab.clone()],
        ),
        &le(&add(&a, &c), &add(&b, &c)),
    );
    s.check(
        &call(
            theory.int_le_sub,
            vec![a.clone(), b.clone(), c.clone(), h_ab.clone()],
        ),
        &le(&sub(&a, &c), &sub(&b, &c)),
    );
    let nonneg = s.assume(le(&Term::int(0), &c));
    s.check(
        &call(
            theory.int_mul_le_mul_nonneg,
            vec![a.clone(), b.clone(), c.clone(), h_ab, nonneg],
        ),
        &le(&mul(&a, &c), &mul(&b, &c)),
    );

    // At literals, with the premises decided by evaluation.
    let decide = |prop: &Term| Term::proof(Proof::Evaluate(prop.clone()));
    let (x, y, z) = (Term::int(-7), Term::int(3), Term::int(5));
    s.check(
        &call(
            theory.int_le_of_lt,
            vec![x.clone(), y.clone(), decide(&lt(&x, &y))],
        ),
        &le(&x, &y),
    );
    s.check(
        &call(
            theory.int_lt_of_le_of_ne,
            vec![
                x.clone(),
                y.clone(),
                decide(&le(&x, &y)),
                decide(&int_eq(&x, &y)),
            ],
        ),
        &lt(&x, &y),
    );
    s.check(
        &call(
            theory.int_le_add_left,
            vec![x.clone(), y.clone(), z.clone(), decide(&le(&x, &y))],
        ),
        &le(&add(&z, &x), &add(&z, &y)),
    );
    s.check(
        &call(
            theory.int_le_add_right,
            vec![x.clone(), y.clone(), z.clone(), decide(&le(&x, &y))],
        ),
        &le(&add(&x, &z), &add(&y, &z)),
    );
    s.check(
        &call(
            theory.int_le_sub,
            vec![x.clone(), y.clone(), z.clone(), decide(&le(&x, &y))],
        ),
        &le(&sub(&x, &z), &sub(&y, &z)),
    );
    s.check(
        &call(
            theory.int_mul_le_mul_nonneg,
            vec![
                x.clone(),
                y.clone(),
                z.clone(),
                decide(&le(&x, &y)),
                decide(&le(&Term::int(0), &z)),
            ],
        ),
        &le(&mul(&x, &z), &mul(&y, &z)),
    );
}

/// A lemma proves its conclusion and nothing else: the wrong conclusion,
/// a premise of the wrong shape, and an argument of the wrong type are
/// each refused.
#[test]
fn a_lemma_is_refused_at_the_wrong_claim_or_argument() {
    let (definitions, prelude, theory) = setup();
    let family = theory.machine(U32);
    let mut s = Scene::new(&definitions, prelude);
    let (a, b) = (s.var(Type::Machine(U32)), s.var(Type::Machine(U32)));
    let (va, vb) = (view(U32, &a), view(U32, &b));
    let strict = s.assume(lt(&va, &vb));
    let proof = call(family.le_of_lt, vec![a.clone(), b.clone(), strict.clone()]);
    // The conclusion is `v(a) <= v(b)`, not `v(b) <= v(a)` and not strict.
    assert!(matches!(
        check_proof(&mut s.ctx, &proof, &le(&vb, &va)),
        Err(KernelError::ProofMismatch { .. })
    ));
    assert!(matches!(
        check_proof(&mut s.ctx, &proof, &lt(&va, &vb)),
        Err(KernelError::ProofMismatch { .. })
    ));
    // The premise must be the strict comparison, not the weak one.
    let weak = s.assume(le(&va, &vb));
    assert!(
        s.infer(&call(family.le_of_lt, vec![a.clone(), b.clone(), weak]))
            .is_err()
    );
    // An argument of another type.
    let other = s.var(Type::Machine(I32));
    assert!(
        s.infer(&call(
            family.le_of_lt,
            vec![other, b.clone(), strict.clone()]
        ))
        .is_err()
    );
    // The lemma at another type does not take these arguments.
    assert!(
        s.infer(&call(
            theory.machine(U16).le_of_lt,
            vec![a.clone(), b.clone(), strict]
        ))
        .is_err()
    );
}

/// Every lemma is a short proof, and the counts are reported.
#[test]
#[doc = "spec: 2.14:8"]
fn every_lemma_is_short() {
    let (definitions, _, theory) = setup();
    let mut widest = 0;
    for (name, id) in theory.lemma_names() {
        let (_, body) = definitions.function_body(id).expect("declared");
        let Term::Proof(proof) = body else {
            panic!("{name}: the body of a lemma is a proof");
        };
        let count = nodes(proof);
        println!("{name}: {count} nodes");
        widest = widest.max(count);
        // The lemmas that go through the table, `succ_le_of_lt` and the
        // two about differences, carry a model step and two certificates
        // for each row they speak of.
        assert!(count <= 28, "{name} has {count} nodes");
    }
    assert!(widest > 1);
}

#[test]
fn the_theory_declares_quickly() {
    let started = Instant::now();
    let (_, _, theory) = setup();
    let elapsed = started.elapsed();
    println!(
        "theory declared in {elapsed:?}: {} named lemmas",
        theory.lemma_names().len()
    );
    assert!(elapsed.as_secs() < 5);
}

// --- The comparisons ---------------------------------------------------------------------

/// What Rust says of a pair of values of a machine type.
fn rust_cmp(op: CmpOp, a: i128, b: i128) -> bool {
    match op {
        CmpOp::Eq => a == b,
        CmpOp::Lt => a < b,
        CmpOp::Le => a <= b,
    }
}

/// The kernel's `evaluate` of a comparison of two literals, as a bool.
fn kernel_cmp(ctx: &mut Context, op: CmpOp, ty: MachineInt, a: i128, b: i128) -> bool {
    let term = cmp(op, ty, &lit(ty, a), &lit(ty, b));
    match infer_proof(ctx, &Proof::Evaluate(term.clone())) {
        Ok(Term::Eq(Type::Bool, left, right)) if *left == term => match *right {
            Term::Bool(flag) => flag,
            other => panic!("{other} is not a bool"),
        },
        other => panic!("{other:?} for {term}"),
    }
}

/// Every pair of values of `u8` and of `i8`, at each comparison, through
/// the native evaluation `literal` and `evaluate` are computed by; and the
/// two rules themselves at a sample of the pairs.
#[test]
#[doc = "spec: 2.13:1, 2.13:2, 2.13:6, 2.13:7"]
fn cmp_agrees_with_rust_on_every_pair_of_bytes() {
    let (definitions, _, _) = setup();
    let mut ctx = Context::with_definitions(definitions);
    for ty in [U8, I8] {
        let (lo, hi) = (ty.min().to_i128().unwrap(), ty.max().to_i128().unwrap());
        for a in lo..=hi {
            for b in lo..=hi {
                for op in CmpOp::ALL {
                    let (x, y) = (lit(ty, a), lit(ty, b));
                    let flag = rust_cmp(op, a, b);
                    assert_eq!(
                        evaluate_primitive(Prim::Cmp(op, ty), &[x.clone(), y.clone()]),
                        Some(Term::Bool(flag)),
                        "{}[{}]({a}, {b})",
                        op.name(),
                        ty.name()
                    );
                    if (a - lo) % 17 == 0 && (b - lo) % 13 == 0 {
                        let term = cmp(op, ty, &x, &y);
                        let expected = Term::eq(Type::Bool, term.clone(), Term::Bool(flag));
                        assert_eq!(
                            infer_proof(&mut ctx, &Proof::Literal(term.clone())),
                            Ok(expected.clone()),
                            "{term}"
                        );
                        assert_eq!(infer_proof(&mut ctx, &Proof::Evaluate(term)), Ok(expected));
                    }
                }
            }
        }
    }
}

#[test]
fn cmp_agrees_with_rust_at_the_boundaries_of_every_type() {
    let (definitions, _, _) = setup();
    let mut ctx = Context::with_definitions(definitions);
    for ty in MachineInt::FIXED {
        let values = boundary(ty);
        for &a in &values {
            for &b in &values {
                for op in CmpOp::ALL {
                    assert_eq!(
                        kernel_cmp(&mut ctx, op, ty, a, b),
                        rust_cmp(op, a, b),
                        "{}[{}]({a}, {b})",
                        op.name(),
                        ty.name()
                    );
                }
            }
        }
    }
}

#[test]
fn cmp_is_typed_at_its_type_in_either_mode_and_refused_otherwise() {
    let (definitions, _, _) = setup();
    let mut ctx = Context::with_definitions(definitions);
    let a = Term::var(ctx.declare(Type::Machine(U16)).unwrap());
    let b = Term::var(ctx.declare(Type::Machine(U16)).unwrap());
    let other = Term::var(ctx.declare(Type::Machine(I16)).unwrap());
    let byte = Term::var(ctx.declare(Type::U8).unwrap());
    let comparison = cmp(CmpOp::Lt, U16, &a, &b);
    // Runtime data in, a bool out: executable.
    assert_eq!(
        infer_term(&mut ctx, &comparison, Mode::Executable),
        Ok(Type::Bool)
    );
    assert_eq!(
        infer_term(&mut ctx, &comparison, Mode::Logical),
        Ok(Type::Bool)
    );
    // `Type::U8` is the type of `cmp[u8]`, as `Type::machine` arranges.
    let at_u8 = cmp(CmpOp::Eq, U8, &byte, &Term::U8(3));
    assert_eq!(
        infer_term(&mut ctx, &at_u8, Mode::Executable),
        Ok(Type::Bool)
    );
    // An operand of another type, of another width, and the wrong arity.
    assert!(infer_term(&mut ctx, &cmp(CmpOp::Lt, U16, &a, &other), Mode::Logical).is_err());
    assert!(infer_term(&mut ctx, &cmp(CmpOp::Lt, U32, &a, &b), Mode::Logical).is_err());
    assert!(infer_term(&mut ctx, &cmp(CmpOp::Le, U8, &byte, &a), Mode::Logical).is_err());
    assert_eq!(
        infer_term(
            &mut ctx,
            &Term::prim(Prim::Cmp(CmpOp::Le, U16), vec![a.clone()]),
            Mode::Logical
        ),
        Err(KernelError::WrongArity {
            expected: 2,
            found: 1
        })
    );
    // A literal of the wrong type has no value under the comparison.
    let mixed = cmp(CmpOp::Eq, U16, &lit(U16, 1), &lit(U32, 1));
    assert!(infer_proof(&mut ctx, &Proof::Literal(mixed)).is_err());
    // The display form names the comparison and its type.
    assert_eq!(format!("{}", Prim::Cmp(CmpOp::Le, I64)), "le[i64]");
    assert_eq!(Prim::Cmp(CmpOp::Eq, U8).name(), "eq");
    assert_eq!(Prim::Cmp(CmpOp::Lt, U8).name(), "lt");
}

// --- cmp_reflect ----------------------------------------------------------------------------

/// Each instance of `cmp_reflect`, at every type and comparison, used in
/// both directions, and near-missed: the wrong type, the wrong flag, the
/// strict comparison claimed for the weak one, and a comparison that is
/// not `Prim::Cmp`.
#[test]
#[doc = "spec: 2.13:3, 2.13:4, 2.13:5"]
fn cmp_reflect_is_used_both_ways_at_every_type_and_near_missed() {
    let (definitions, prelude, _) = setup();
    for ty in MachineInt::FIXED {
        for op in CmpOp::ALL {
            let mut s = Scene::new(&definitions, prelude);
            let over = Type::machine(ty);
            let (a, b) = (s.var(over.clone()), s.var(over.clone()));
            let (va, vb) = (view(ty, &a), view(ty, &b));
            let claim = op.claim(va.clone(), vb.clone());
            let comparison = cmp(op, ty, &a, &b);

            // From `c == true`, the claim; from `c == false`, its negation.
            let observed_true = s.assume_proof(is(&comparison, true));
            let from_true = Proof::implies_elim(
                Proof::Axiom(Axiom::CmpReflect(comparison.clone(), true)),
                observed_true,
            );
            s.check(&from_true, &claim);
            let observed_false = s.assume_proof(is(&comparison, false));
            let from_false = Proof::implies_elim(
                Proof::Axiom(Axiom::CmpReflect(comparison.clone(), false)),
                observed_false,
            );
            s.check(&from_false, &s.not(claim.clone()));

            // The statements themselves.
            assert_eq!(
                s.infer(&Proof::Axiom(Axiom::CmpReflect(comparison.clone(), true))),
                Ok(Term::implies(is(&comparison, true), claim.clone()))
            );
            assert_eq!(
                s.infer(&Proof::Axiom(Axiom::CmpReflect(comparison.clone(), false))),
                Ok(Term::implies(is(&comparison, false), s.not(claim.clone())))
            );

            // Near misses. The wrong flag proves the other direction.
            let negated = s.not(claim.clone());
            assert!(matches!(
                check_proof(&mut s.ctx, &from_true, &negated),
                Err(KernelError::ProofMismatch { .. })
            ));
            assert!(matches!(
                check_proof(&mut s.ctx, &from_false, &claim),
                Err(KernelError::ProofMismatch { .. })
            ));
            // The strict comparison claimed for the weak one, and the other
            // way round.
            let sibling = match op {
                CmpOp::Le => CmpOp::Lt,
                CmpOp::Lt => CmpOp::Le,
                CmpOp::Eq => CmpOp::Le,
            };
            assert!(matches!(
                check_proof(
                    &mut s.ctx,
                    &from_true,
                    &sibling.claim(va.clone(), vb.clone())
                ),
                Err(KernelError::ProofMismatch { .. })
            ));
            // The wrong type: operands of another type, or the axiom at a
            // comparison of another type than its operands.
            let neighbour = if ty == U8 { U16 } else { U8 };
            let foreign = cmp(op, neighbour, &a, &b);
            assert!(
                s.infer(&Proof::Axiom(Axiom::CmpReflect(foreign, true)))
                    .is_err()
            );
            // The views at another type are a different claim.
            let elsewhere = op.claim(view(neighbour, &a), view(neighbour, &b));
            assert!(check_proof(&mut s.ctx, &from_true, &elsewhere).is_err());
            // A comparison that is not `Prim::Cmp`: a bool literal.
            assert_eq!(
                s.infer(&Proof::Axiom(Axiom::CmpReflect(Term::Bool(true), true))),
                Err(KernelError::NoComputationStep(Term::Bool(true)))
            );
            // The name.
            assert_eq!(Axiom::CmpReflect(comparison, true).name(), "cmp_reflect");
        }
    }
}

/// `cmp_reflect` at literals, in both directions, across the whole range at
/// 8 bits: what evaluation computes for the comparison is what evaluation
/// decides for the views, and the axiom connects the two.
#[test]
fn cmp_reflect_agrees_with_evaluation_on_every_pair_of_bytes() {
    let (definitions, prelude, _) = setup();
    let mut s = Scene::new(&definitions, prelude);
    for ty in [U8, I8] {
        let (lo, hi) = (ty.min().to_i128().unwrap(), ty.max().to_i128().unwrap());
        for a in (lo..=hi).step_by(7) {
            for b in (lo..=hi).step_by(5) {
                for op in CmpOp::ALL {
                    let (x, y) = (lit(ty, a), lit(ty, b));
                    let comparison = cmp(op, ty, &x, &y);
                    let claim = op.claim(view(ty, &x), view(ty, &y));
                    let flag = rust_cmp(op, a, b);
                    let proof = Proof::implies_elim(
                        Proof::Axiom(Axiom::CmpReflect(comparison.clone(), flag)),
                        Proof::Evaluate(comparison.clone()),
                    );
                    let expected = if flag { claim } else { s.not(claim) };
                    s.check(&proof, &expected);
                    // Evaluation decides the same about the views.
                    s.check(
                        &Proof::Evaluate(op.claim(view(ty, &x), view(ty, &y))),
                        &expected,
                    );
                }
            }
        }
    }
}

/// The lemmas at the ends of a range: `view_bounds` and `lt_irrefl` at
/// `max`, and `cmp` at `min` against `max`, at every type, with the numbers
/// of the table.
#[test]
fn the_bounds_and_the_ends_of_every_range() {
    let (definitions, prelude, theory) = setup();
    for ty in MachineInt::FIXED {
        let mut s = Scene::new(&definitions, prelude);
        let family = theory.machine(ty);
        let (min, max) = (
            lit(ty, ty.min().to_i128().unwrap()),
            lit(ty, ty.max().to_i128().unwrap()),
        );
        let both = call(family.view_bounds, vec![max.clone()]);
        let claim = prelude.and_prop(
            le(&Term::Int(ty.min()), &view(ty, &max)),
            le(&view(ty, &max), &Term::Int(ty.max())),
        );
        s.check(&both, &claim);
        // `min < max` at the runtime comparison, hence over the views.
        let strict = cmp(CmpOp::Lt, ty, &min, &max);
        s.check(
            &call(
                family.lt_of_cmp,
                vec![
                    min.clone(),
                    max.clone(),
                    Term::proof(Proof::Evaluate(strict)),
                ],
            ),
            &lt(&view(ty, &min), &view(ty, &max)),
        );
        // And `max < min` is refuted by the false comparison.
        let reversed = cmp(CmpOp::Le, ty, &max, &min);
        s.check(
            &call(
                family.lt_of_not_le,
                vec![
                    max.clone(),
                    min.clone(),
                    Term::proof(Proof::Evaluate(reversed)),
                ],
            ),
            &lt(&view(ty, &min), &view(ty, &max)),
        );
    }
}
