//! Tests for the rule `linear` (build task K6; the kernel contract in
//! atlas.html): a certificate of linear arithmetic over `Int`.
//!
//! The certificates of the contract's worked example, the last obligation of
//! `midpoint`, are built by hand exactly as written there, and so are
//! certificates for the other arithmetic obligations of the target examples.
//! A test-side checker recomputes each combination in `i128` over the same
//! reading, and every one-step change of each midpoint certificate is
//! decided by both. Fabricated constraints are rejected as bad proofs and not
//! as bad arithmetic. Random false goals over a small box are never
//! accepted; set LOCUS_EXTENDED to run a hundred times as many. Atoms are
//! merged exactly when the kernel calls them the same term, and each limit
//! has a test at it. Every term here is written by hand.

use std::collections::BTreeMap;
use std::rc::Rc;

use locus::kernel::{
    Axiom, CertificateText, Context, Definitions, HypId, HypRef, Integer, KernelError, LinearError,
    MAX_LINEAR_ATOMS, MAX_LINEAR_BITS, MAX_LINEAR_PAIRS, MachineInt, Mode, Prelude, Prim, Proof,
    Term, Type, check_proof, infer_proof, infer_term,
};

#[path = "common/rng.rs"]
mod rng;
use rng::{Rng, case_seed};

use MachineInt::{I8, U16, U32};

fn setup() -> (Context, Prelude) {
    let (definitions, prelude) = Definitions::with_prelude();
    (Context::with_definitions(Rc::new(definitions)), prelude)
}

fn extended() -> bool {
    std::env::var_os("LOCUS_EXTENDED").is_some()
}

fn lit(value: i128) -> Term {
    Term::Int(Integer::from(value))
}

fn add(left: Term, right: Term) -> Term {
    Term::int_add(left, right)
}

fn sub(left: Term, right: Term) -> Term {
    Term::int_sub(left, right)
}

fn mul(left: Term, right: Term) -> Term {
    Term::int_mul(left, right)
}

fn div(left: Term, right: Term) -> Term {
    Term::int_div(left, right)
}

fn rem(left: Term, right: Term) -> Term {
    Term::int_rem(left, right)
}

fn le(left: Term, right: Term) -> Term {
    Term::int_le(left, right)
}

fn lt(left: Term, right: Term) -> Term {
    Term::int_lt(left, right)
}

fn eq(left: Term, right: Term) -> Term {
    Term::eq(Type::Int, left, right)
}

fn view(ty: MachineInt, x: &Term) -> Term {
    Term::view(ty, x.clone())
}

fn ax(axiom: Axiom) -> Proof {
    Proof::Axiom(axiom)
}

fn mp(implication: Proof, premise: Proof) -> Proof {
    Proof::implies_elim(implication, premise)
}

fn linear(goal: &Term, c0: i64, pairs: Vec<(Proof, i64)>) -> Proof {
    Proof::linear(goal.clone(), c0, pairs)
}

fn linear_error(result: Result<Term, KernelError>) -> LinearError {
    match result {
        Err(KernelError::Linear(error)) => error,
        other => panic!("expected the linear rule to refuse, found {other:?}"),
    }
}

fn accepted(ctx: &mut Context, proof: &Proof, goal: &Term) {
    check_proof(ctx, proof, goal).unwrap_or_else(|error| panic!("{error}"));
}

/// The literal `2^bits`, which has `bits + 1` bits.
fn power_of_two(bits: usize) -> Integer {
    let two = Integer::from(2i64);
    (0..bits).fold(Integer::from(1i64), |acc, _| acc.mul(&two))
}

/// The sum of the terms as a balanced tree, so that a long sum stays
/// shallow.
fn balanced_sum(terms: &[Term]) -> Term {
    match terms {
        [] => lit(0),
        [one] => one.clone(),
        _ => {
            let (left, right) = terms.split_at(terms.len() / 2);
            add(balanced_sum(left), balanced_sum(right))
        }
    }
}

// --- A test-side reading in i128 ----------------------------------------------------

/// A linear form with atoms keyed by the printed term. This is written
/// apart from the kernel's reading and shares nothing with it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Form {
    constant: i128,
    atoms: BTreeMap<String, i128>,
}

impl Form {
    fn scaled(&self, by: i128) -> Form {
        Form {
            constant: self.constant.checked_mul(by).expect("small numbers"),
            atoms: self
                .atoms
                .iter()
                .map(|(atom, c)| (atom.clone(), c.checked_mul(by).expect("small numbers")))
                .filter(|(_, c)| *c != 0)
                .collect(),
        }
    }

    fn plus(&self, other: &Form) -> Form {
        let mut atoms = self.atoms.clone();
        for (atom, c) in &other.atoms {
            let total = atoms.get(atom).copied().unwrap_or(0) + c;
            if total == 0 {
                atoms.remove(atom);
            } else {
                atoms.insert(atom.clone(), total);
            }
        }
        Form {
            constant: self.constant + other.constant,
            atoms,
        }
    }

    fn minus(&self, other: &Form) -> Form {
        self.plus(&other.scaled(-1))
    }

    fn atom(term: &Term) -> Form {
        Form {
            constant: 0,
            atoms: BTreeMap::from([(term.to_string(), 1)]),
        }
    }
}

fn read(term: &Term) -> Form {
    let pair = |arguments: &[Term]| (read(&arguments[0]), read(&arguments[1]));
    match term {
        Term::Int(value) => Form {
            constant: value.to_i128().expect("small literals"),
            atoms: BTreeMap::new(),
        },
        Term::Prim(Prim::IntAdd, arguments) => {
            let (l, r) = pair(arguments);
            l.plus(&r)
        }
        Term::Prim(Prim::IntSub, arguments) => {
            let (l, r) = pair(arguments);
            l.minus(&r)
        }
        Term::Prim(Prim::IntNeg, arguments) => read(&arguments[0]).scaled(-1),
        Term::Prim(Prim::IntMul, arguments) => {
            let (l, r) = pair(arguments);
            if l.atoms.is_empty() {
                r.scaled(l.constant)
            } else if r.atoms.is_empty() {
                l.scaled(r.constant)
            } else {
                Form::atom(term)
            }
        }
        _ => Form::atom(term),
    }
}

fn comparison(prop: &Term) -> Option<(&Term, &Term)> {
    match prop {
        Term::Prim(Prim::IntLe, arguments) => Some((&arguments[0], &arguments[1])),
        _ => None,
    }
}

/// The contribution of a proved constraint times its coefficient, or `None`
/// when the pair is not admissible.
fn contribution(prop: &Term, c: i128, falsehood: &Term) -> Option<Form> {
    if let Some((s, t)) = comparison(prop) {
        return (c >= 0).then(|| read(t).minus(&read(s)).scaled(c));
    }
    if let Term::Eq(Type::Int, s, t) = prop {
        return Some(read(t).minus(&read(s)).scaled(c));
    }
    if let Term::Implies(premise, conclusion) = prop
        && let Some((s, t)) = comparison(premise)
        && **conclusion == *falsehood
    {
        let mut form = read(s).minus(&read(t));
        form.constant -= 1;
        return (c >= 0).then(|| form.scaled(c));
    }
    None
}

/// Whether the certificate is valid: the negated goal times `c0` plus the
/// pairs sum to a negative constant. Each pair is a conclusion, not a proof.
fn valid(goal: &Term, c0: i128, pairs: &[(&Term, i128)], falsehood: &Term) -> bool {
    let mut sum = Form::default();
    if let Some((s, t)) = comparison(goal) {
        if c0 <= 0 {
            return false;
        }
        let mut negated = read(s).minus(&read(t));
        negated.constant -= 1;
        sum = negated.scaled(c0);
    } else if goal != falsehood {
        return false;
    }
    for (prop, c) in pairs {
        match contribution(prop, *c, falsehood) {
            Some(form) => sum = sum.plus(&form),
            None => return false,
        }
    }
    sum.atoms.is_empty() && sum.constant < 0
}

// --- The midpoint example ------------------------------------------------------------

/// A proved constraint: its proof, and its conclusion as the test writes
/// it, which the test-side checker reads instead of asking the kernel.
struct Fact {
    name: &'static str,
    proof: Proof,
    claim: Term,
}

/// The context of the last obligation of `midpoint`, with the facts of the
/// contract's table and a few more. The machine values `hi - lo`, `half`,
/// and `mid` are variables `d`, `half`, and `mid` of `u32`, and the exact
/// results of the three operations, known because their obligations were
/// met, are hypotheses about their views; the bridge from `lo <= hi` to
/// `L <= H` is a hypothesis too, since K8's lemma does not exist yet.
struct Midpoint {
    ctx: Context,
    prelude: Prelude,
    facts: Vec<Fact>,
    lo: Term,
    hi: Term,
    l: Term,
    h: Term,
    f: Term,
    m: Term,
    q_of_sum: Term,
}

const MAX_U32: i128 = 4294967295;

fn midpoint() -> Midpoint {
    let (mut ctx, prelude) = setup();
    let u32 = Type::machine(U32);
    let var = |ctx: &mut Context| Term::var(ctx.declare(u32.clone()).unwrap());
    let (lo, hi, d, half, mid) = (
        var(&mut ctx),
        var(&mut ctx),
        var(&mut ctx),
        var(&mut ctx),
        var(&mut ctx),
    );
    let (l, h, dv, f, m) = (
        view(U32, &lo),
        view(U32, &hi),
        view(U32, &d),
        view(U32, &half),
        view(U32, &mid),
    );
    let (q, r) = (div(dv.clone(), lit(2)), rem(dv.clone(), lit(2)));
    let s = add(l.clone(), h.clone());
    let (big_q, big_r) = (div(s.clone(), lit(2)), rem(s.clone(), lit(2)));

    let ord = ctx.assume(le(l.clone(), h.clone())).unwrap();
    let sub_exact = ctx
        .assume(eq(dv.clone(), sub(h.clone(), l.clone())))
        .unwrap();
    let div_exact = ctx.assume(eq(f.clone(), q.clone())).unwrap();
    let add_exact = ctx
        .assume(eq(m.clone(), add(l.clone(), f.clone())))
        .unwrap();

    let two_pos = Proof::Evaluate(lt(lit(0), lit(2)));
    let lo0 = ax(Axiom::ViewLower(U32, lo.clone()));
    let hi0 = ax(Axiom::ViewLower(U32, hi.clone()));
    let e3_condition = linear(&le(lit(0), s.clone()), 1, vec![(lo0.clone(), 1), (hi0, 1)]);
    let again = |claim: &Term, proof: Proof| {
        Proof::implies_elim(Proof::implies_intro(claim.clone(), |h| h), proof)
    };

    let mut facts = Vec::new();
    let mut fact = |name, proof: Proof, claim: Term| {
        facts.push(Fact { name, proof, claim });
    };
    fact("ord", Proof::hyp(ord), le(l.clone(), h.clone()));
    fact("lo0", lo0, le(lit(0), l.clone()));
    fact(
        "hi1",
        ax(Axiom::ViewUpper(U32, hi.clone())),
        le(h.clone(), lit(MAX_U32)),
    );
    fact(
        "sub",
        Proof::hyp(sub_exact),
        eq(dv.clone(), sub(h.clone(), l.clone())),
    );
    fact("div", Proof::hyp(div_exact), eq(f.clone(), q.clone()));
    fact(
        "add",
        Proof::hyp(add_exact),
        eq(m.clone(), add(l.clone(), f.clone())),
    );
    fact(
        "d1",
        ax(Axiom::IntDivRem(dv.clone(), lit(2))),
        eq(dv.clone(), add(mul(q.clone(), lit(2)), r.clone())),
    );
    fact(
        "d2",
        mp(
            ax(Axiom::IntRemUpperPos(dv.clone(), lit(2))),
            two_pos.clone(),
        ),
        lt(r.clone(), lit(2)),
    );
    let d3 = mp(
        ax(Axiom::IntRemNonneg(dv.clone(), lit(2))),
        ax(Axiom::ViewLower(U32, d.clone())),
    );
    fact("d3", d3.clone(), le(lit(0), r.clone()));
    fact(
        "e1",
        ax(Axiom::IntDivRem(s.clone(), lit(2))),
        eq(s.clone(), add(mul(big_q.clone(), lit(2)), big_r.clone())),
    );
    fact(
        "e2",
        mp(ax(Axiom::IntRemUpperPos(s.clone(), lit(2))), two_pos),
        lt(big_r.clone(), lit(2)),
    );
    fact(
        "e3",
        mp(ax(Axiom::IntRemNonneg(s.clone(), lit(2))), e3_condition),
        le(lit(0), big_r.clone()),
    );
    // The same facts by other proofs, so that replacing a pair can leave a
    // certificate valid; and facts the certificates do not use.
    fact(
        "ord_again",
        again(&le(l.clone(), h.clone()), Proof::hyp(ord)),
        le(l.clone(), h.clone()),
    );
    fact(
        "d3_again",
        again(&le(lit(0), r.clone()), d3),
        le(lit(0), r.clone()),
    );
    fact(
        "hi0",
        ax(Axiom::ViewLower(U32, hi.clone())),
        le(lit(0), h.clone()),
    );
    fact(
        "lo1",
        ax(Axiom::ViewUpper(U32, lo.clone())),
        le(l.clone(), lit(MAX_U32)),
    );
    fact(
        "d0",
        ax(Axiom::ViewLower(U32, d.clone())),
        le(lit(0), dv.clone()),
    );
    fact(
        "f1",
        ax(Axiom::ViewUpper(U32, half.clone())),
        le(f.clone(), lit(MAX_U32)),
    );
    fact(
        "m0",
        ax(Axiom::ViewLower(U32, mid.clone())),
        le(lit(0), m.clone()),
    );

    Midpoint {
        ctx,
        prelude,
        facts,
        lo,
        hi,
        l,
        h,
        f,
        m,
        q_of_sum: big_q,
    }
}

impl Midpoint {
    fn fact(&self, name: &str) -> usize {
        self.facts
            .iter()
            .position(|fact| fact.name == name)
            .unwrap_or_else(|| panic!("no fact {name}"))
    }

    fn certificate(&self, goal: &Term, c0: i64, pairs: &[(&str, i64)]) -> Certificate {
        Certificate {
            goal: goal.clone(),
            c0,
            pairs: pairs
                .iter()
                .map(|(name, c)| (self.fact(name), *c))
                .collect(),
        }
    }

    fn proof(&self, certificate: &Certificate) -> Proof {
        linear(
            &certificate.goal,
            certificate.c0,
            certificate
                .pairs
                .iter()
                .map(|(fact, c)| (self.facts[*fact].proof.clone(), *c))
                .collect(),
        )
    }

    fn valid(&self, certificate: &Certificate) -> bool {
        let pairs: Vec<(&Term, i128)> = certificate
            .pairs
            .iter()
            .map(|(fact, c)| (&self.facts[*fact].claim, i128::from(*c)))
            .collect();
        valid(
            &certificate.goal,
            i128::from(certificate.c0),
            &pairs,
            &self.prelude.falsehood_prop(),
        )
    }

    /// The four certificates of the contract, in its order, and the one
    /// behind `e3`.
    fn contract_certificates(&self) -> Vec<(&'static str, Certificate)> {
        let (l, h, f, m, q) = (&self.l, &self.h, &self.f, &self.m, &self.q_of_sum);
        vec![
            (
                "hi - lo",
                self.certificate(&le(lit(0), sub(h.clone(), l.clone())), 1, &[("ord", 1)]),
            ),
            (
                "lo + half",
                self.certificate(
                    &le(add(l.clone(), f.clone()), lit(MAX_U32)),
                    2,
                    &[
                        ("div", 2),
                        ("d1", -1),
                        ("sub", 1),
                        ("ord", 1),
                        ("hi1", 2),
                        ("d3", 1),
                    ],
                ),
            ),
            (
                "M <= Q",
                self.certificate(
                    &le(m.clone(), q.clone()),
                    2,
                    &[
                        ("add", 2),
                        ("div", 2),
                        ("d1", -1),
                        ("sub", 1),
                        ("e1", 1),
                        ("e2", 1),
                        ("d3", 1),
                    ],
                ),
            ),
            (
                "Q <= M",
                self.certificate(
                    &le(q.clone(), m.clone()),
                    2,
                    &[
                        ("add", -2),
                        ("div", -2),
                        ("d1", 1),
                        ("sub", -1),
                        ("e1", -1),
                        ("d2", 1),
                        ("e3", 1),
                    ],
                ),
            ),
            (
                "0 <= L + H",
                self.certificate(
                    &le(lit(0), add(l.clone(), h.clone())),
                    1,
                    &[("lo0", 1), ("hi0", 1)],
                ),
            ),
        ]
    }
}

#[derive(Clone, Debug)]
struct Certificate {
    goal: Term,
    c0: i64,
    pairs: Vec<(usize, i64)>,
}

#[test]
#[doc = "spec: 2.15:1, 2.15:11, 2.15:2, 2.15:3, 2.15:4, 2.15:6, 2.15:7, 2.15:8, 2.15:9, 2.16:1, 2.16:10, 2.16:11, 2.16:12, 2.16:14, 2.16:15, 2.16:3, 2.16:4, 2.16:5, 2.16:6, 2.16:7, 2.16:8"]
fn the_midpoint_certificates_of_the_contract_are_accepted() {
    let mut example = midpoint();
    for (name, certificate) in example.contract_certificates() {
        assert!(
            example.valid(&certificate),
            "{name} is valid on the test side"
        );
        let proof = example.proof(&certificate);
        check_proof(&mut example.ctx, &proof, &certificate.goal)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
    }
    // The claim itself, M == Q, by antisymmetry from the two halves.
    let certificates = example.contract_certificates();
    let [_, _, (_, m_le_q), (_, q_le_m), _] = certificates.as_slice() else {
        unreachable!("five certificates")
    };
    let (m, q) = (example.m.clone(), example.q_of_sum.clone());
    let claim = eq(m.clone(), q.clone());
    let proof = mp(
        mp(ax(Axiom::IntLeAntisymm(m, q)), example.proof(m_le_q)),
        example.proof(q_le_m),
    );
    accepted(&mut example.ctx, &proof, &claim);
    // The conclusion is the goal, and a different one is a mismatch.
    let (name, half) = &certificates[1];
    let proof = example.proof(half);
    assert_eq!(
        infer_proof(&mut example.ctx, &proof).unwrap(),
        half.goal,
        "{name}"
    );
    let other = le(add(example.l.clone(), example.f.clone()), lit(MAX_U32 - 1));
    assert!(matches!(
        check_proof(&mut example.ctx, &proof, &other),
        Err(KernelError::ProofMismatch { .. })
    ));
}

/// The other arithmetic obligations of the target examples: the lock's
/// `fits`, `next.failures <= 3` from it, and the two in `remaining`. The
/// midpoint's `hi - lo` and `lo + half` are in the contract's four above.
/// A machine result whose obligation was met is a variable with a
/// hypothesis about its view, and `u32::MAX as Int` is the literal.
#[test]
fn the_other_obligations_of_the_target_examples_are_accepted() {
    let (mut ctx, prelude) = setup();
    let falsehood = prelude.falsehood_prop();
    let u32 = Type::machine(U32);
    let check =
        |ctx: &mut Context, name: &str, goal: Term, c0: i64, pairs: Vec<(Proof, Term, i64)>| {
            let facts: Vec<(&Term, i128)> = pairs
                .iter()
                .map(|(_, claim, c)| (claim, i128::from(*c)))
                .collect();
            assert!(valid(&goal, i128::from(c0), &facts, &falsehood), "{name}");
            let proof = linear(
                &goal,
                c0,
                pairs
                    .iter()
                    .map(|(proof, _, c)| (proof.clone(), *c))
                    .collect(),
            );
            check_proof(ctx, &proof, &goal).unwrap_or_else(|error| panic!("{name}: {error}"));
        };

    // The lock: from lock.failures < 3, its view plus one fits in u32.
    let failures = Term::var(ctx.declare(u32.clone()).unwrap());
    let fv = view(U32, &failures);
    let below = lt(fv.clone(), lit(3));
    let small = ctx.assume(below.clone()).unwrap();
    check(
        &mut ctx,
        "fits",
        le(add(fv.clone(), lit(1)), lit(MAX_U32)),
        1,
        vec![(Proof::hyp(small), below.clone(), 1)],
    );
    // next.failures <= 3, where next.failures is lock.failures + 1 exactly.
    let next = Term::var(ctx.declare(u32.clone()).unwrap());
    let nv = view(U32, &next);
    let exact = eq(nv.clone(), add(fv.clone(), lit(1)));
    let exact_h = ctx.assume(exact.clone()).unwrap();
    check(
        &mut ctx,
        "next.failures <= 3",
        le(nv, lit(3)),
        1,
        vec![
            (Proof::hyp(exact_h), exact, 1),
            (Proof::hyp(small), below, 1),
        ],
    );
    // remaining: 3 - failures does not go below zero, from failures <= 3.
    let bounded = le(fv.clone(), lit(3));
    let bounded_h = ctx.assume(bounded.clone()).unwrap();
    check(
        &mut ctx,
        "3 - failures",
        le(lit(0), sub(lit(3), fv.clone())),
        1,
        vec![(Proof::hyp(bounded_h), bounded, 1)],
    );
    // remaining: left <= 3, where left is 3 - failures exactly, from the
    // range of failures.
    let left = Term::var(ctx.declare(u32).unwrap());
    let lv = view(U32, &left);
    let exact = eq(lv.clone(), sub(lit(3), fv.clone()));
    let exact_h = ctx.assume(exact.clone()).unwrap();
    check(
        &mut ctx,
        "left <= 3",
        le(lv, lit(3)),
        1,
        vec![
            (Proof::hyp(exact_h), exact, 1),
            (ax(Axiom::ViewLower(U32, failures)), le(lit(0), fv), 1),
        ],
    );
}

// --- Every one-step change of the midpoint certificates ---------------------------------

/// Every one-step change: each coefficient up, down, negated, and zeroed,
/// the goal's coefficient likewise, each pair dropped, and each pair
/// replaced by each other available fact.
fn mutants(example: &Midpoint, certificate: &Certificate) -> Vec<Certificate> {
    let mut out = Vec::new();
    let steps = |c: i64| [c + 1, c - 1, -c, 0];
    for c0 in steps(certificate.c0) {
        out.push(Certificate {
            c0,
            ..certificate.clone()
        });
    }
    for at in 0..certificate.pairs.len() {
        for c in steps(certificate.pairs[at].1) {
            let mut changed = certificate.clone();
            changed.pairs[at].1 = c;
            out.push(changed);
        }
        let mut dropped = certificate.clone();
        dropped.pairs.remove(at);
        out.push(dropped);
        for fact in 0..example.facts.len() {
            if fact != certificate.pairs[at].0 {
                let mut replaced = certificate.clone();
                replaced.pairs[at].0 = fact;
                out.push(replaced);
            }
        }
    }
    out
}

#[test]
fn the_kernel_accepts_exactly_the_valid_mutants_of_the_midpoint_certificates() {
    let mut example = midpoint();
    let (mut valid_count, mut invalid_count) = (0, 0);
    for (name, certificate) in example.contract_certificates() {
        for mutant in mutants(&example, &certificate) {
            let expected = example.valid(&mutant);
            let proof = example.proof(&mutant);
            let result = check_proof(&mut example.ctx, &proof, &mutant.goal);
            assert_eq!(
                result.is_ok(),
                expected,
                "{name}: c0 = {}, pairs {:?}: kernel {result:?}, test side {expected}",
                mutant.c0,
                mutant
                    .pairs
                    .iter()
                    .map(|(fact, c)| (example.facts[*fact].name, *c))
                    .collect::<Vec<_>>()
            );
            if expected {
                valid_count += 1;
            } else {
                invalid_count += 1;
            }
        }
    }
    println!("midpoint mutants: {valid_count} valid and {invalid_count} invalid, all agreeing");
    assert!(valid_count > 0, "some mutants stay valid");
    assert!(invalid_count > valid_count);
}

// --- Fabricated constraints ----------------------------------------------------------

#[test]
fn a_range_fact_with_no_proof_is_rejected_as_a_bad_proof() {
    let mut example = midpoint();
    let goal = le(example.l.clone(), lit(MAX_U32));
    // A bare claim has no way in: the only proof of a fact from nowhere is
    // a hypothesis that is not in the context.
    let missing = HypId::fresh();
    let proof = linear(&goal, 1, vec![(Proof::Hyp(HypRef::Free(missing)), 1)]);
    assert_eq!(
        infer_proof(&mut example.ctx, &proof),
        Err(KernelError::UnknownHypothesis(missing))
    );
    // With the proof, the range fact is admitted.
    let proof = linear(
        &goal,
        1,
        vec![(ax(Axiom::ViewUpper(U32, example.lo.clone())), 1)],
    );
    accepted(&mut example.ctx, &proof, &goal);
}

#[test]
fn a_range_fact_at_the_wrong_type_is_rejected_as_a_bad_proof() {
    let mut example = midpoint();
    let goal = le(example.h.clone(), lit(65535));
    // view_upper[u16] at a u32 variable: the axiom instance is ill typed.
    let proof = linear(
        &goal,
        1,
        vec![(ax(Axiom::ViewUpper(U16, example.hi.clone())), 1)],
    );
    assert!(matches!(
        infer_proof(&mut example.ctx, &proof),
        Err(KernelError::TypeMismatch { .. })
    ));
}

#[test]
fn a_quotient_constraint_for_another_divisor_does_not_cancel() {
    let mut example = midpoint();
    // 0 <= D / 2 from the decomposition and the sign of the remainder is
    // fine with the decomposition at 2, and the atoms of the decomposition
    // at 3 are other terms, so the sum keeps an atom.
    let variable = Term::var(example.ctx.declare(Type::machine(U32)).unwrap());
    let d = view(U32, &variable);
    let goal = le(lit(0), div(d.clone(), lit(2)));
    let nonneg = ax(Axiom::ViewLower(U32, variable));
    let bound = mp(
        ax(Axiom::IntRemUpperPos(d.clone(), lit(2))),
        Proof::Evaluate(lt(lit(0), lit(2))),
    );
    // With the divisor 2: 2(-q - 1) + (2q + r - D) + D + (1 - r) = -1.
    let pairs = |divisor: i128| {
        vec![
            (ax(Axiom::IntDivRem(d.clone(), lit(divisor))), 1),
            (nonneg.clone(), 1),
            (bound.clone(), 1),
        ]
    };
    accepted(&mut example.ctx, &linear(&goal, 2, pairs(2)), &goal);
    let error = linear_error(infer_proof(&mut example.ctx, &linear(&goal, 2, pairs(3))));
    let LinearError::Uncancelled(atom) = error else {
        panic!("expected an uncancelled atom, found {error}");
    };
    assert_eq!(atom, div(d, lit(2)), "the goal's quotient is left over");
}

#[test]
fn a_remainder_bound_without_its_condition_is_not_a_constraint() {
    let mut example = midpoint();
    let d = view(U32, &example.lo);
    let r = rem(d.clone(), lit(2));
    let goal = le(r.clone(), lit(1));
    // The implication `0 < 2 => r < 2` is not eliminated, so what the pair
    // proves is an implication, which is not one of the three constraints.
    let proof = linear(
        &goal,
        1,
        vec![(ax(Axiom::IntRemUpperPos(d.clone(), lit(2))), 1)],
    );
    let error = linear_error(infer_proof(&mut example.ctx, &proof));
    assert!(matches!(error, LinearError::NotAConstraint(_)), "{error}");
    // Eliminated, it is.
    let proof = linear(
        &goal,
        1,
        vec![(
            mp(
                ax(Axiom::IntRemUpperPos(d, lit(2))),
                Proof::Evaluate(lt(lit(0), lit(2))),
            ),
            1,
        )],
    );
    accepted(&mut example.ctx, &proof, &goal);
}

#[test]
#[doc = "spec: 2.15:10"]
fn the_shapes_of_goals_and_coefficients_are_checked() {
    let (mut ctx, prelude) = setup();
    let x = Term::var(ctx.declare(Type::Int).unwrap());
    let h = Proof::hyp(ctx.assume(le(x.clone(), lit(5))).unwrap());
    let goal = le(x.clone(), lit(5));
    accepted(&mut ctx, &linear(&goal, 1, vec![(h.clone(), 1)]), &goal);

    // An equation is not a goal, nor a negated inequality, nor a non-proposition.
    for bad in [
        eq(x.clone(), lit(5)),
        prelude.not_prop(le(lit(6), x.clone())),
        prelude.truth_prop(),
    ] {
        let error = linear_error(infer_proof(
            &mut ctx,
            &linear(&bad, 1, vec![(h.clone(), 1)]),
        ));
        assert!(matches!(error, LinearError::NotAGoal(_)), "{bad}: {error}");
    }
    let ill_typed = le(x.clone(), Term::Bool(true));
    assert!(matches!(
        infer_proof(&mut ctx, &linear(&ill_typed, 1, vec![(h.clone(), 1)])),
        Err(KernelError::TypeMismatch { .. })
    ));
    // The goal's coefficient must be positive.
    for c0 in [0, -1] {
        let error = linear_error(infer_proof(
            &mut ctx,
            &linear(&goal, c0, vec![(h.clone(), 1)]),
        ));
        assert!(matches!(error, LinearError::GoalCoefficient(_)), "{error}");
    }
    // An inequality's coefficient must not be negative.
    let error = linear_error(infer_proof(
        &mut ctx,
        &linear(&goal, 1, vec![(h.clone(), -1)]),
    ));
    assert!(
        matches!(error, LinearError::NegativeCoefficient(_)),
        "{error}"
    );
    // A sum that cancels to a non-negative constant proves nothing:
    // x <= 5 from x <= 6 is -(x - 5 - 1) ... (x - 6) + (6 - x) = 0.
    let weaker = Proof::hyp(ctx.assume(le(x.clone(), lit(6))).unwrap());
    let error = linear_error(infer_proof(&mut ctx, &linear(&goal, 1, vec![(weaker, 1)])));
    assert_eq!(error, LinearError::NotNegative(Integer::zero()));
    // A hypothesis that is not a constraint.
    let other = Proof::hyp(ctx.assume(prelude.truth_prop()).unwrap());
    let error = linear_error(infer_proof(&mut ctx, &linear(&goal, 1, vec![(other, 1)])));
    assert!(matches!(error, LinearError::NotAConstraint(_)), "{error}");
    // The False goal ignores its coefficient; the pairs must contradict.
    let lower = Proof::hyp(ctx.assume(le(lit(6), x.clone())).unwrap());
    let falsehood = prelude.falsehood_prop();
    accepted(
        &mut ctx,
        &linear(&falsehood, 0, vec![(h.clone(), 1), (lower, 1)]),
        &falsehood,
    );
    let error = linear_error(infer_proof(&mut ctx, &linear(&falsehood, 1, vec![(h, 1)])));
    assert!(matches!(error, LinearError::Uncancelled(_)), "{error}");
    // With no pairs at all, a goal that is a tautology on literals holds.
    let trivial = le(lit(3), lit(4));
    accepted(&mut ctx, &linear(&trivial, 1, vec![]), &trivial);
    let error = linear_error(infer_proof(
        &mut ctx,
        &linear(&le(lit(4), lit(3)), 1, vec![]),
    ));
    assert_eq!(error, LinearError::NotNegative(Integer::zero()));
}

#[test]
fn the_rule_needs_no_prelude_for_a_comparison_goal() {
    let mut ctx = Context::new();
    let x = Term::var(ctx.declare(Type::Int).unwrap());
    let h = Proof::hyp(ctx.assume(le(x.clone(), lit(5))).unwrap());
    let goal = le(x, lit(5));
    accepted(&mut ctx, &linear(&goal, 1, vec![(h, 1)]), &goal);
}

// --- Random goals on a small box -------------------------------------------------------

/// A goal `sum c_i x_i + k <= sum d_i x_i + k'` over `i8` variables, whose
/// two sides are random terms.
struct Domain {
    ctx: Context,
    prelude: Prelude,
    /// The views of the variables.
    atoms: Vec<Term>,
    /// Each fact with its coefficient vector and constant, read as
    /// `constant + sum coefficient * x >= 0`.
    facts: Vec<(Proof, Term, Vec<i128>, i128)>,
}

const BOUND: i128 = 8;

fn random_domain(variables: usize) -> Domain {
    let (mut ctx, prelude) = setup();
    let mut atoms = Vec::new();
    let mut facts = Vec::new();
    for index in 0..variables {
        let x = Term::var(ctx.declare(Type::machine(I8)).unwrap());
        let v = view(I8, &x);
        let unit = |c: i128| {
            let mut row = vec![0; variables];
            row[index] = c;
            row
        };
        // -8 <= x and x <= 8 as hypotheses, and the range of i8 as axioms.
        let lower = le(lit(-BOUND), v.clone());
        let lower_h = ctx.assume(lower.clone()).unwrap();
        facts.push((Proof::hyp(lower_h), lower, unit(1), BOUND));
        let upper = le(v.clone(), lit(BOUND));
        let upper_h = ctx.assume(upper.clone()).unwrap();
        facts.push((Proof::hyp(upper_h), upper, unit(-1), BOUND));
        facts.push((
            ax(Axiom::ViewLower(I8, x.clone())),
            le(lit(-128), v.clone()),
            unit(1),
            128,
        ));
        facts.push((
            ax(Axiom::ViewUpper(I8, x.clone())),
            le(v.clone(), lit(127)),
            unit(-1),
            127,
        ));
        atoms.push(v);
    }
    Domain {
        ctx,
        prelude,
        atoms,
        facts,
    }
}

/// A random side: a constant and a coefficient in -3..=3 for each variable,
/// written as a term in a random order of operations.
fn random_side(rng: &mut Rng, atoms: &[Term]) -> (Vec<i128>, i128, Term) {
    let coefficients: Vec<i128> = (0..atoms.len())
        .map(|_| rng.range(0..7) as i128 - 3)
        .collect();
    let constant = rng.range(0..21) as i128 - 10;
    let mut term = lit(constant);
    for (atom, c) in atoms.iter().zip(&coefficients) {
        let scaled = match c {
            0 => continue,
            1 => atom.clone(),
            -1 => Term::int_neg(atom.clone()),
            c if rng.chance(1, 2) => mul(lit(*c), atom.clone()),
            c => mul(atom.clone(), lit(*c)),
        };
        term = if rng.chance(1, 2) {
            add(term, scaled)
        } else {
            sub(term, Term::int_neg(scaled))
        };
    }
    (coefficients, constant, term)
}

/// Whether `left <= right` holds at every point of the box `[-8, 8]^n`.
fn true_on_box(left: (&[i128], i128), right: (&[i128], i128)) -> bool {
    let n = left.0.len();
    let mut point = vec![-BOUND; n];
    loop {
        let value =
            |(cs, k): (&[i128], i128)| k + cs.iter().zip(&point).map(|(c, x)| c * x).sum::<i128>();
        if value(left) > value(right) {
            return false;
        }
        let mut carry = 0;
        while carry < n {
            point[carry] += 1;
            if point[carry] <= BOUND {
                break;
            }
            point[carry] = -BOUND;
            carry += 1;
        }
        if carry == n {
            return true;
        }
    }
}

#[test]
fn no_random_certificate_proves_a_false_goal_on_the_box() {
    const SEED: u64 = 0x6b6c_2024_0001;
    let goals = if extended() { 10_000 } else { 100 };
    let per_goal = 1000;
    let mut checked = 0;
    let mut goal_count = 0;
    let mut case = 0;
    while goal_count < goals {
        let mut rng = Rng::new(case_seed(SEED, case));
        case += 1;
        let mut world = random_domain(rng.range(2..5));
        let (lc, lk, left) = random_side(&mut rng, &world.atoms);
        let (rc, rk, right) = random_side(&mut rng, &world.atoms);
        if true_on_box((&lc, lk), (&rc, rk)) {
            continue;
        }
        goal_count += 1;
        let goal = le(left, right);
        infer_term(&mut world.ctx, &goal, Mode::Logical).unwrap();
        for _ in 0..per_goal {
            let count = rng.range(1..7);
            let pairs: Vec<(Proof, i64)> = (0..count)
                .map(|_| {
                    let fact = rng.choose(&world.facts);
                    let c = if rng.chance(1, 8) {
                        -(rng.range(1..4) as i64)
                    } else {
                        rng.range(0..4) as i64
                    };
                    (fact.0.clone(), c)
                })
                .collect();
            let c0 = rng.range(1..4) as i64;
            let proof = linear(&goal, c0, pairs);
            checked += 1;
            if let Ok(claim) = infer_proof(&mut world.ctx, &proof) {
                panic!("case {case}: a false goal {claim} was accepted");
            }
        }
    }
    println!("{checked} random certificates for {goal_count} false goals, none accepted");
    assert_eq!(checked, goals * per_goal);
}

/// A certificate for a true goal on the box, by a tiny search: up to four
/// of the bound hypotheses with coefficients 1..=3, and `c0 = 1`.
fn search(world: &Domain, goal: (&[i128], i128, &[i128], i128)) -> Option<Vec<(usize, i64)>> {
    let (lc, lk, rc, rk) = goal;
    let n = lc.len();
    // The negated goal: left - right - 1 >= 0.
    let base: Vec<i128> = (0..n).map(|i| lc[i] - rc[i]).collect();
    let base_k = lk - rk - 1;
    let hyps: Vec<usize> = (0..world.facts.len()).filter(|i| i % 4 < 2).collect();
    let mut chosen: Vec<(usize, i64)> = Vec::new();
    fn go(
        world: &Domain,
        hyps: &[usize],
        from: usize,
        chosen: &mut Vec<(usize, i64)>,
        row: &[i128],
        k: i128,
    ) -> bool {
        if row.iter().all(|c| *c == 0) && k < 0 {
            return true;
        }
        if chosen.len() == 4 {
            return false;
        }
        for (at, fact) in hyps.iter().enumerate().skip(from) {
            for c in 1..=3i64 {
                let (_, _, coefficients, constant) = &world.facts[*fact];
                let row: Vec<i128> = row
                    .iter()
                    .zip(coefficients)
                    .map(|(r, f)| r + f * i128::from(c))
                    .collect();
                chosen.push((*fact, c));
                if go(
                    world,
                    hyps,
                    at + 1,
                    chosen,
                    &row,
                    k + constant * i128::from(c),
                ) {
                    return true;
                }
                chosen.pop();
            }
        }
        false
    }
    go(world, &hyps, 0, &mut chosen, &base, base_k).then_some(chosen)
}

#[test]
fn a_certificate_found_for_a_true_goal_on_the_box_is_accepted() {
    const SEED: u64 = 0x6b6c_2024_0002;
    let wanted = if extended() { 5_000 } else { 50 };
    let (mut found, mut case) = (0, 0);
    while found < wanted {
        let mut rng = Rng::new(case_seed(SEED, case));
        case += 1;
        let mut world = random_domain(rng.range(2..5));
        let (lc, lk, left) = random_side(&mut rng, &world.atoms);
        let (rc, rk, right) = random_side(&mut rng, &world.atoms);
        if !true_on_box((&lc, lk), (&rc, rk)) {
            continue;
        }
        let Some(pairs) = search(&world, (&lc, lk, &rc, rk)) else {
            continue;
        };
        found += 1;
        let goal = le(left, right);
        let proof = linear(
            &goal,
            1,
            pairs
                .iter()
                .map(|(fact, c)| (world.facts[*fact].0.clone(), *c))
                .collect(),
        );
        let claims: Vec<(&Term, i128)> = pairs
            .iter()
            .map(|(fact, c)| (&world.facts[*fact].1, i128::from(*c)))
            .collect();
        assert!(valid(&goal, 1, &claims, &world.prelude.falsehood_prop()));
        check_proof(&mut world.ctx, &proof, &goal)
            .unwrap_or_else(|error| panic!("case {case}: {error}"));
    }
    println!("{found} true goals with a certificate found, all accepted, in {case} cases");
}

// --- Atoms --------------------------------------------------------------------------

#[test]
#[doc = "spec: 2.15:5"]
fn atoms_are_merged_exactly_when_they_are_the_same_term() {
    let (mut ctx, _) = setup();
    let x = Term::var(ctx.declare(Type::Int).unwrap());
    let y = Term::var(ctx.declare(Type::Int).unwrap());
    let m = Term::var(ctx.declare(Type::machine(U32)).unwrap());
    let x_small = Proof::hyp(ctx.assume(le(x.clone(), lit(5))).unwrap());
    // Two variables are never merged: the certificate would be valid if
    // they were.
    let goal = le(y.clone(), lit(5));
    let error = linear_error(infer_proof(
        &mut ctx,
        &linear(&goal, 1, vec![(x_small.clone(), 1)]),
    ));
    assert!(matches!(error, LinearError::Uncancelled(_)), "{error}");
    // Two occurrences of view(m), built separately, are one atom.
    let v_small = Proof::hyp(ctx.assume(le(view(U32, &m), lit(5))).unwrap());
    let goal = le(view(U32, &m), lit(5));
    accepted(&mut ctx, &linear(&goal, 1, vec![(v_small, 1)]), &goal);
    // 2 * x, x * 2, and x + x read to the same form.
    let twice = Proof::hyp(ctx.assume(le(mul(lit(2), x.clone()), lit(9))).unwrap());
    for goal in [
        le(add(x.clone(), x.clone()), lit(9)),
        le(mul(x.clone(), lit(2)), lit(9)),
        le(sub(mul(lit(3), x.clone()), x.clone()), lit(9)),
        le(Term::int_neg(mul(lit(-2), x.clone())), lit(9)),
    ] {
        accepted(&mut ctx, &linear(&goal, 1, vec![(twice.clone(), 1)]), &goal);
    }
    // A product of two atoms is an atom, and the rule does not commute
    // inside it: x * y and y * x are different atoms.
    let product = Proof::hyp(ctx.assume(le(mul(x.clone(), y.clone()), lit(5))).unwrap());
    let goal = le(mul(x.clone(), y.clone()), lit(5));
    accepted(
        &mut ctx,
        &linear(&goal, 1, vec![(product.clone(), 1)]),
        &goal,
    );
    let goal = le(mul(y.clone(), x.clone()), lit(5));
    let error = linear_error(infer_proof(&mut ctx, &linear(&goal, 1, vec![(product, 1)])));
    assert!(matches!(error, LinearError::Uncancelled(_)), "{error}");
    // A quotient is an atom, read through a multiplication by a literal.
    let q = div(x.clone(), lit(3));
    let q_small = Proof::hyp(ctx.assume(le(mul(q.clone(), lit(2)), lit(5))).unwrap());
    let goal = le(add(q.clone(), q.clone()), lit(5));
    accepted(&mut ctx, &linear(&goal, 1, vec![(q_small, 1)]), &goal);
    // An atom whose coefficient cancels is dropped: x - x is the constant
    // 0, so (x - x) * y is 0 and needs no fact.
    let goal = le(mul(sub(x.clone(), x.clone()), y.clone()), lit(0));
    accepted(&mut ctx, &linear(&goal, 1, vec![]), &goal);
    // A literal times a literal is a constant, and it is not 0 * y.
    let goal = le(mul(lit(2), lit(3)), lit(6));
    accepted(&mut ctx, &linear(&goal, 1, vec![]), &goal);
    let goal = le(mul(lit(2), y.clone()), lit(6));
    let error = linear_error(infer_proof(&mut ctx, &linear(&goal, 1, vec![])));
    assert!(matches!(error, LinearError::Uncancelled(_)), "{error}");
}

// --- Limits ----------------------------------------------------------------------------

#[test]
#[doc = "spec: 2.15:12"]
fn the_number_of_pairs_is_limited() {
    let (mut ctx, _) = setup();
    let x = Term::var(ctx.declare(Type::Int).unwrap());
    let h = Proof::hyp(ctx.assume(le(x.clone(), lit(5))).unwrap());
    let goal = le(x, lit(5));
    let pairs = |count: usize| -> Vec<(Proof, i64)> {
        (0..count)
            .map(|index| (h.clone(), i64::from(index == 0)))
            .collect()
    };
    accepted(&mut ctx, &linear(&goal, 1, pairs(MAX_LINEAR_PAIRS)), &goal);
    assert_eq!(
        linear_error(infer_proof(
            &mut ctx,
            &linear(&goal, 1, pairs(MAX_LINEAR_PAIRS + 1))
        )),
        LinearError::TooManyPairs(MAX_LINEAR_PAIRS + 1)
    );
}

#[test]
#[doc = "spec: 2.15:12"]
fn the_number_of_atoms_in_a_form_is_limited() {
    let (mut ctx, _) = setup();
    // -1 <= x_1 + ... + x_n from 0 <= x_1 + ... + x_n: the form of either
    // side has n atoms, and there is one pair.
    fn sum_of(ctx: &mut Context, count: usize) -> (Term, Vec<(Proof, i64)>) {
        let vars: Vec<Term> = (0..count)
            .map(|_| Term::var(ctx.declare(Type::Int).unwrap()))
            .collect();
        let sum = balanced_sum(&vars);
        let nonneg = Proof::hyp(ctx.assume(le(lit(0), sum.clone())).unwrap());
        (le(lit(-1), sum), vec![(nonneg, 1)])
    }
    let (goal, pairs) = sum_of(&mut ctx, MAX_LINEAR_ATOMS);
    accepted(&mut ctx, &linear(&goal, 1, pairs), &goal);
    let (goal, pairs) = sum_of(&mut ctx, MAX_LINEAR_ATOMS + 1);
    assert_eq!(
        linear_error(infer_proof(&mut ctx, &linear(&goal, 1, pairs))),
        LinearError::TooManyAtoms(MAX_LINEAR_ATOMS + 1)
    );
}

#[test]
#[doc = "spec: 2.15:12"]
fn the_size_of_literals_is_limited() {
    let (mut ctx, _) = setup();
    let x = Term::var(ctx.declare(Type::Int).unwrap());
    let h = Proof::hyp(ctx.assume(le(x.clone(), lit(5))).unwrap());
    let goal = le(x.clone(), lit(5));
    let at_limit = power_of_two(MAX_LINEAR_BITS - 1);
    let past = power_of_two(MAX_LINEAR_BITS);
    assert_eq!(at_limit.magnitude().bit_length(), MAX_LINEAR_BITS);
    // A coefficient at the limit: c0 (x - 6) + c0 (5 - x) = -c0.
    let with = |c0: &Integer, c: &Integer| Proof::Linear {
        goal: goal.clone(),
        goal_coefficient: c0.clone(),
        pairs: vec![(h.clone(), c.clone())],
    };
    accepted(&mut ctx, &with(&at_limit, &at_limit), &goal);
    assert_eq!(
        linear_error(infer_proof(&mut ctx, &with(&past, &at_limit))),
        LinearError::LiteralTooLarge(past.clone())
    );
    assert_eq!(
        linear_error(infer_proof(&mut ctx, &with(&Integer::from(1i64), &past))),
        LinearError::LiteralTooLarge(past.clone())
    );
    // A literal inside the goal, and inside a hypothesis.
    let wide = |n: &Integer| le(x.clone(), Term::Int(n.clone()));
    accepted(
        &mut ctx,
        &linear(&wide(&at_limit), 1, vec![(h.clone(), 1)]),
        &wide(&at_limit),
    );
    assert_eq!(
        linear_error(infer_proof(
            &mut ctx,
            &linear(&wide(&past), 1, vec![(h.clone(), 1)])
        )),
        LinearError::LiteralTooLarge(past.clone())
    );
    let huge = Proof::hyp(ctx.assume(wide(&past)).unwrap());
    assert_eq!(
        linear_error(infer_proof(&mut ctx, &linear(&goal, 1, vec![(huge, 1)]))),
        LinearError::LiteralTooLarge(past)
    );
}

// --- The text form ---------------------------------------------------------------------

#[test]
#[doc = "spec: 2.15:13"]
fn a_certificate_has_a_text_form_with_a_version() {
    let example = midpoint();
    let (_, certificate) = &example.contract_certificates()[0];
    let proof = example.proof(certificate);
    let text = CertificateText(&proof).to_string();
    assert!(text.starts_with("linear v1 "), "{text}");
    assert!(text.ends_with(" ; 1 ; [hyp * 1]"), "{text}");
    assert!(text.contains(&certificate.goal.to_string()), "{text}");
    let (_, certificate) = &example.contract_certificates()[1];
    let text = CertificateText(&example.proof(certificate)).to_string();
    let pairs = concat!(
        "[hyp * 2, axiom int_div_rem * -1, hyp * 1, hyp * 1, ",
        "axiom view_upper * 2, implies_elim * 1]"
    );
    assert!(text.ends_with(pairs), "{text}");
    assert_eq!(CertificateText(&Proof::Omitted).to_string(), "");
}
