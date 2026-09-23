//! Quantifier values use checked named propositions and logical callables.
use locus::{
    elab::{Elaborated, Options, elaborate_with_options},
    parser::parse,
    source::SourceMap,
};
fn check(text: &str) -> Elaborated {
    let mut sources = SourceMap::default();
    let file = sources.add("quantifier_library.lc", text);
    let source = sources.get(file);
    let parsed = parse(source);
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
    let mut options = Options::default();
    if locus::preview::Feature::LogicalData.status() == locus::preview::Status::Preview {
        options.previews.enable("logical-data").unwrap();
    }
    elaborate_with_options(source, &parsed.program, &options)
}
fn accept(text: &str) {
    let result = check(text);
    assert!(result.is_success(), "{:#?}", result.diagnostics);
    assert_eq!(locus::erased::check_module(result.session.erased()), Ok(()));
}
#[test]
fn existential_constructor_carries_a_witness_and_checked_evidence() {
    accept(
        "logic fn exists_zero() -> @Exists::<Int>(|n: Int| prop!(n == 0)) {
        Exists::<Int>::Witness(0) @ prove!(0 == 0)
    }",
    );
}
#[test]
fn universal_constructor_carries_a_checked_proof_function() {
    accept(
        "logic fn reflexivity() -> @ForAll::<Int>(|n: Int| prop!(n == n)) {
        ForAll::<Int>::Each(|n: Int| prove!(n == n)) @ True::Intro
    }",
    );
}
#[test]
fn existential_witness_is_available_within_a_proof_match() {
    accept("logic fn use_exists(predicate: logic Fn(n: Int) -> Prop, h: @Exists::<Int>(predicate)) -> @Exists::<Int>(predicate) {
        match h { Exists::Witness(n) @ proof => Exists::<Int>::Witness(n) @ proof }
    }");
}
#[test]
#[doc = "spec: 1.25:4"]
fn universal_proof_function_can_be_applied_to_a_particular_input() {
    accept("logic fn at(predicate: logic Fn(n: Int) -> Prop, h: @ForAll::<Int>(predicate), n: Int) -> @predicate(n) {
        match h { ForAll::Each(prove_each) @ _ => prove_each(n) }
    }");
}
#[test]
#[doc = "spec: 1.25:4"]
fn existential_witness_cannot_escape_as_logical_data() {
    let result = check(
        "logic fn bad(predicate: logic Fn(n: Int) -> Prop, h: @Exists::<Int>(predicate)) -> Int {
        match h { Exists::Witness(n) @ _ => n }
    }",
    );
    assert!(!result.is_success());
}

#[test]
fn quantifier_keyword_sugar_uses_the_same_constructors() {
    accept(
        "logic fn reflexivity() -> @(forall(n: Int) { n == n }) {
        ForAll::<Int>::Each(|n: Int| prove!(n == n)) @ True::Intro
    }
    logic fn witness() -> @(exists(n: Int) { n == 0 }) {
        Exists::<Int>::Witness(0) @ prove!(0 == 0)
    }",
    );
}
#[test]
fn universal_evidence_application_uses_checked_elimination() {
    accept("logic fn specialize(h: @(forall(n: Int) { n == n })) -> @(3 == 3) { h(3) }");
}

#[test]
fn named_theorem_lifts_into_library_universal_evidence() {
    accept(
        "logic fn refl(n: Int) -> @(n == n) { prove!(n == n) }
        logic fn theorem() -> @(forall(n: Int) { n == n }) { refl }",
    );
}

#[test]
fn nested_quantifiers_and_implications_lift_a_named_theorem() {
    accept(
        "logic fn from_q(p: Prop, q: Prop, hq: @q, hp: @p) -> @q { hq }
      fn apply(p: Prop, q: Prop, hq: @q) -> @(p => q) {
        let all: @(forall(a: Prop) { forall(b: Prop) { b => a => b } }) = from_q;
        all(p)(q)(hq)
      }",
    );
}

#[test]
#[doc = "spec: 1.6:7, 2.34:3"]
fn default_source_quantifiers_have_only_library_proposition_heads() {
    use locus::{
        kernel::{Term, Type},
        typed::FnRef,
    };
    let result = check("logic fn universal() -> @(forall(a: Int) { forall(b: Int) { a == a && b == b } }) {
      ForAll::<Int>::Each(|a: Int| ForAll::<Int>::Each(|b: Int| And::Intro(prove!(a == a), prove!(b == b))) @ True::Intro) @ True::Intro
    }");
    assert!(result.is_success(), "{:#?}", result.diagnostics);
    let Some(FnRef::Math(id)) = result.function("universal") else {
        panic!("logical function")
    };
    let Some(Type::Fn(_, ty)) = result.session.program().definitions().signature(id) else {
        panic!("signature")
    };
    let Type::Proof(claim) = *ty else {
        panic!("proof result")
    };
    assert!(matches!(*claim, Term::PropApp(..)));
    assert!(
        claim
            .find(&|term| matches!(term, Term::Forall(..) | Term::Exists(..)))
            .is_none()
    );
}

#[test]
fn quantification_requires_a_logical_domain() {
    let result = check("logic fn invalid() -> @(forall(n: u8) { n == n }) { _ }");
    assert!(!result.is_success());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("Logical"))
    );
}
