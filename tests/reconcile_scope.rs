//! Snapshots carry values across scopes; exposed proof types may not leave
//! their local binders behind. These checks exercise the logical-split gate.

use locus::elab::{Elaborated, Options, elaborate_with_options};
use locus::erased::{Interpreter, Outcome, Value};
use locus::preview::{Feature, Status};
use locus::source::SourceMap;

fn check(text: &str) -> Elaborated {
    let mut sources = SourceMap::default();
    let file = sources.add("scope.lc", text);
    let source = sources.get(file);
    let parsed = locus::parser::parse(source);
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let mut options = Options::default();
    if Feature::LogicalSplit.status() == Status::Preview {
        options.previews.enable("logical-split").unwrap();
    }
    elaborate_with_options(source, &parsed.program, &options)
}

fn accepts(text: &str, name: &str, expected: u8) {
    let checked = check(text);
    assert!(checked.is_success(), "{:?}", checked.diagnostics);
    let function = checked.function(name).unwrap();
    assert_eq!(
        Interpreter::new(checked.session.erased(), 100_000)
            .call(function, vec![])
            .unwrap(),
        Outcome::Value(Value::u8(expected))
    );
}

#[test]
#[doc = "spec: 1.6:1"]
fn a_stored_claim_and_its_evidence_can_escape_an_opaque_local() {
    accepts(
        r#"
struct Certificate { claim: Prop, evidence: @claim }
fn pick() -> u8 { 7 }
fn packaged() -> u8 {
    let certificate = {
        let hidden = pick();
        let claim = logic { prop!((hidden as Int) == (hidden as Int)) };
        Certificate { claim, evidence: prove!(claim) }
    };
    let carried: @(model!(certificate.claim)) = certificate.evidence;
    7
}
"#,
        "packaged",
        7,
    );
}

#[test]
#[doc = "spec: 1.6:1"]
fn a_bare_proof_cannot_expose_an_opaque_block_local() {
    let checked = check(
        r#"
fn pick() -> u8 { 7 }
fn leak() -> u8 {
    let escaped = {
        let hidden = pick();
        let claim = logic { prop!((hidden as Int) == (hidden as Int)) };
        prove!(claim)
    };
    0
}
"#,
    );
    assert!(!checked.is_success());
    assert!(
        checked
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "L0280"),
        "{:?}",
        checked.diagnostics
    );
    assert!(
        !checked
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "L0300"),
        "{:?}",
        checked.diagnostics
    );
}

#[test]
fn a_total_local_alias_can_be_removed_with_checked_proof_transport() {
    accepts(
        r#"
fn alias(n: u8) -> u8 {
    let proof = {
        let local = n;
        prove!((local as Int) == (n as Int))
    };
    let checked: @((n as Int) == (n as Int)) = proof;
    n
}
fn run() -> u8 { alias(8) }
"#,
        "run",
        8,
    );
}

#[test]
#[doc = "spec: 1.2:1, 1.6:1"]
fn captured_claims_survive_mutation_and_shadowing() {
    accepts(
        r#"
fn captured() -> u8 {
    let mut n = 7u8;
    let claim = logic { prop!((n as Int) == 7) };
    let before: @claim = _;
    n = 0;
    let n = 9u8;
    let still: @claim = before;
    n
}
"#,
        "captured",
        9,
    );
}

#[test]
fn an_unrelated_runtime_field_does_not_rebind_the_stored_claim() {
    accepts(
        r#"
struct Annotated { value: u8, claim: Prop, evidence: @claim }
fn changed() -> u8 {
    let claim = logic { prop!(3 == 3) };
    let mut note = Annotated { value: 3, claim, evidence: prove!(claim) };
    note.value = 0;
    let still: @(model!(note.claim)) = note.evidence;
    note.value
}
"#,
        "changed",
        0,
    );
}

#[test]
fn replacing_a_claim_without_its_dependent_evidence_is_rejected() {
    let checked = check(
        r#"
struct Annotated { value: u8, claim: Prop, evidence: @claim }
fn changed() -> u8 {
    let claim = logic { prop!(3 == 3) };
    let mut note = Annotated { value: 3, claim, evidence: prove!(claim) };
    note.claim = logic { prop!(4 == 4) };
    note.value
}
"#,
    );
    assert!(!checked.is_success());
    assert!(
        checked
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "L0233"
                || diagnostic.message.contains("depends")
                || diagnostic.message.contains("dependent")),
        "{:?}",
        checked.diagnostics
    );
}

#[test]
fn a_wrong_version_names_the_assignment_that_separated_the_values() {
    let checked = check(
        r#"
fn stale(mut n: u8, earlier: @((n as Int) == 7)) -> () {
    n = 9;
    let current: @((n as Int) == 7) = earlier;
}
"#,
    );
    assert!(!checked.is_success());
    assert!(
        checked
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("before line 3")
                || diagnostic
                    .notes
                    .iter()
                    .any(|note| note.contains("before line 3"))),
        "{:?}",
        checked.diagnostics
    );
}

#[test]
fn a_claim_captured_before_move_stays_available() {
    accepts(
        r#"
struct Token { value: u8 }
fn consume(token: Token) -> u8 { token.value }
fn run() -> u8 {
    let token = Token { value: 7 };
    let claim = logic { prop!((model!(token.value) as Int) == 7) };
    let before: @claim = _;
    let value = consume(token);
    let after: @claim = before;
    value
}
"#,
        "run",
        7,
    );
}

#[test]
fn a_new_observation_after_move_is_rejected() {
    let checked = check(
        r#"
struct Token { value: u8 }
fn consume(token: Token) -> u8 { token.value }
fn run() -> u8 {
    let token = Token { value: 7 };
    let value = consume(token);
    let claim = logic { prop!((model!(token.value) as Int) == 7) };
    value
}
"#,
    );
    assert!(!checked.is_success());
    assert!(
        checked
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "L0241"),
        "{:?}",
        checked.diagnostics
    );
}
