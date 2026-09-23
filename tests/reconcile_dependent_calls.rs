use locus::{
    elab::{Options, elaborate_with_options},
    parser::parse,
    source::SourceMap,
};
fn check(text: &str) -> locus::elab::Elaborated {
    let mut sources = SourceMap::default();
    let file = sources.add("dependent_calls.lc", text);
    let source = sources.get(file);
    let parsed = parse(source);
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    elaborate_with_options(source, &parsed.program, &Options::default())
}
const DEFINITIONS: &str = "logic fn f(n:Int,h:@(n>=0))->Int {n} logic fn lemma(n:Int,h:@(n>=0))->@(f(n,h)==n) {fold!(f,prove!(n==n))}";
#[test]
fn aliases_transport_data_and_its_dependent_evidence_together() {
    let source = format!(
        "{DEFINITIONS} logic fn checked(n:Int,h:@(n>=0))->@(f(n,h)==n) {{let m=n;let h0:@(m>=0)=h;let result=f(m,h0);let claim=lemma(m,h0);let exact:@(result==m)=claim;exact}}"
    );
    let result = check(&source);
    assert!(result.is_success(), "{:?}", result.diagnostics);
}
#[test]
fn transported_argument_cannot_change_the_result_claim() {
    let source = format!(
        "{DEFINITIONS} logic fn bad(n:Int,h:@(n>=0))->@(f(n,h)==n+1) {{let m=n;let h0:@(m>=0)=h;let claim=lemma(m,h0);claim}}"
    );
    let result = check(&source);
    assert!(!result.is_success());
}
