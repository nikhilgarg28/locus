use locus::{
    elab::{self, Options},
    parser,
    source::SourceMap,
    typed::FnRef,
};
fn check(text: &str) -> elab::Elaborated {
    let mut sources = SourceMap::default();
    let file = sources.add("trusted.lc", text);
    let source = sources.get(file);
    let parsed = parser::parse(source);
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let mut options = Options::default();
    if locus::preview::Feature::HeapViews.status() == locus::preview::Status::Preview {
        options.previews.enable("heap-views").unwrap();
    }
    elab::elaborate_with_options(source, &parsed.program, &options)
}
#[test]
#[doc = "spec: 1.26:3"]
fn false_foreign_specification_is_explicit_audited_and_never_a_kernel_definition() {
    let text = include_str!("corpus/accept/trusted_collection.lc");
    let checked = check(text);
    assert!(checked.is_success(), "{:?}", checked.diagnostics);
    let FnRef::Exec(id) = checked.function("counted").unwrap() else {
        panic!("foreign item entered the logic")
    };
    let records = checked.session.program().trusted_contracts();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].function, id);
    assert!(records[0].reason.contains("deliberately false"));
    let audit = locus::audit::render(&checked);
    assert!(audit.contains("trusted counted = Vec::len"));
    assert!(audit.contains(&records[0].reason));
    let injected = text.replace("fn inspect()", "logic fn inspect()");
    let denied = check(&injected);
    assert!(!denied.is_success());
}
#[test]
fn foreign_shape_cannot_change_physical_return_type() {
    let checked = check(include_str!("corpus/reject/trusted_shape.lc"));
    assert!(
        checked
            .diagnostics
            .iter()
            .any(|d| d.code == "L0287" && d.message.contains("runtime shape")),
        "{:?}",
        checked.diagnostics
    );
}
#[test]
fn reason_is_grammar_not_an_optional_comment() {
    for text in [
        "trusted fn x()->u8 = Vec::len;",
        "trusted \"\" fn x()->u8 = Vec::len;",
    ] {
        let mut sources = SourceMap::default();
        let file = sources.add("reason.lc", text);
        let parsed = parser::parse(sources.get(file));
        assert!(!parsed.is_success());
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|d| d.message.contains("reason"))
        );
    }
}
