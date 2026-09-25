//! Concrete generic instances pass the same typed lowering and kernel checks.
use locus::elab::{Elaborated, Options, elaborate_with_options};
use locus::erased::{Interpreter, check_module};
use locus::parser::parse;
use locus::source::SourceMap;
fn check(text: &str) -> Elaborated {
    let mut sources = SourceMap::default();
    let id = sources.add("generics.lc", text);
    let source = sources.get(id);
    let parsed = parse(source);
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
    let mut options = Options::default();
    for feature in ["logical-split", "named-props", "logical-data"] {
        let feature = locus::preview::Feature::parse(feature).unwrap();
        if feature.status() == locus::preview::Status::Preview {
            options.previews.enable(feature.name()).unwrap();
        }
    }
    elaborate_with_options(source, &parsed.program, &options)
}
fn accepted(text: &str) -> Elaborated {
    let result = check(text);
    assert!(result.is_success(), "{:#?}", result.diagnostics);
    assert_eq!(check_module(result.session.erased()), Ok(()));
    result
}
fn run(text: &str, expected: &str) {
    let result = accepted(text);
    let module = result.session.erased();
    let value = Interpreter::new(module, 10_000)
        .call(result.function("run").unwrap(), vec![])
        .unwrap();
    assert_eq!(value.debug(module), expected);
}
#[test]
fn explicit_and_inferred_function_instances_execute() {
    run(
        "fn identity<T>(x: T) -> T { x } fn run() -> u8 { let a = identity::<u8>(7); identity(a) }",
        "7",
    );
}
#[test]
fn generic_struct_and_enum_payloads_are_checked_and_pattern_types_flow() {
    run("struct Holder<T> { value: T } enum Maybe<T> { Missing, Present(T) }
    fn wrap(x: u8) -> Maybe<Holder<u8>> { Maybe::Present(Holder { value: x }) }
    fn run() -> u8 { match wrap(9) { Maybe::Present(holder) => holder.value, Maybe::Missing => 0 } }", "9");
}
#[test]
#[doc = "spec: 1.3:1"]
fn option_and_result_prelude_keep_runtime_tags() {
    run("fn make() -> Option<u8> { Some(3) }
        fn run() -> u8 { let result: Result<u8, u8> = Result::Ok(4); match make() { Option::Some(n) => n, Option::None => 0 } }", "3");
}
#[test]
fn distinct_source_occurrences_share_an_instance() {
    run(
        "fn identity(x: Option<u8>) -> Option<u8> { x } fn run() -> u8 { let a: Option<u8> = Option::Some(5); match identity(a) { Option::Some(n) => n, Option::None => 0 } }",
        "5",
    );
}
#[test]
fn logical_function_generic_mode_is_explicit() {
    accepted(
        "logic fn identity<T: Logical>(x: T) -> T { x } fn make() -> Int { identity::<Int>(3) }",
    );
    for source in [
        "logic fn identity<T>(x: T) -> T { x } fn bad() -> u8 { identity::<u8>(3) }",
        "logic fn identity<T: Logical>(x: T) -> T { x } fn make() -> u8 { identity::<u8>(3) }",
        "fn identity<T>(x: T) -> T { x } logic fn bad(n: Int) -> Int { identity::<Int>(n) }",
    ] {
        assert!(!check(source).is_success(), "{source}");
    }
}
#[test]
fn instantiated_bodies_are_never_trusted() {
    let checked = check("fn bad<T>(x: T) -> bool { x } fn run() -> bool { bad::<u8>(7) }");
    assert!(!checked.is_success());
    assert!(
        checked.diagnostics.iter().all(|d| d.code != "L0300"),
        "{:#?}",
        checked.diagnostics
    );
}
#[test]
fn generic_proposition_constructors_use_the_same_explicit_evidence_slot() {
    accepted(
        "prop Known<T: Logical>(value: T) { Yes => { prop!(true) } }
        fn run() -> @Known::<Int>(3) { Known::<Int>::Yes @ True::Intro }",
    );
}
#[test]
#[doc = "spec: 1.3:3"]
fn insufficient_inference_and_unsupported_bounds_are_diagnostics() {
    for source in [
        "fn identity<T>(x: T) -> T { x } fn run() -> () { let x = identity(3); }",
        "fn identity<T: Debug>(x: T) -> T { x }",
    ] {
        let checked = check(source);
        assert!(
            checked.diagnostics.iter().any(|d| d.code == "L0281"),
            "{source}\n{:#?}",
            checked.diagnostics
        );
    }
}
#[test]
#[doc = "spec: 1.3:1"]
fn proof_payloads_are_erased_but_option_tag_survives() {
    run("prop Good { Yes => { prop!(true) } }
        fn run() -> u8 { let value: Option<@Good> = Some(Good::Yes @ True::Intro); match value { Option::Some(h) => 7, Option::None => 0 } }", "7");
}

#[test]
fn argument_inference_recurses_into_generic_type_arguments() {
    run(
        "fn unwrap<T>(value: Option<T>, fallback: T) -> T { match value { Option::Some(x) => x, Option::None => fallback } } fn run() -> u8 { let x: Option<u8> = Some(8); let value = unwrap(x, 0u8); value }",
        "8",
    );
}
#[test]
fn early_return_carries_the_function_result_context() {
    run(
        "fn make(x: bool) -> Option<u8> { if x { return Some(3); } else { return None; } } fn run() -> u8 { match make(true) { Option::Some(n) => n, Option::None => 0 } }",
        "3",
    );
}

#[test]
#[doc = "spec: 1.3:3"]
fn template_checking_is_per_instance_and_grouped_arguments_are_identical() {
    // This is intentionally a template, not a theorem for every T. No generated
    // declaration exists until it is instantiated; each instance must check.
    accepted("fn unused<T>(x: T) -> bool { x } fn run() -> u8 { 0 }");
    run(
        "fn id(x: Option<(u8)>) -> Option<u8> { x } fn run() -> u8 { let v: Option<u8> = Some(2); match id(v) { Option::Some(n) => n, Option::None => 0 } }",
        "2",
    );
}

#[test]
fn growing_specialization_is_bounded() {
    let result = check(
        "fn grow<T>(x: T) -> () { grow::<Option<T>>(None); } fn start() -> () { grow::<u8>(0); }",
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "L0281" && d.message.contains("256")),
        "{:#?}",
        result.diagnostics
    );
}

#[path = "common/corpus.rs"]
mod runner;
#[test]
fn generic_bound_rejection_pins_the_preview_and_diagnostic() {
    let name = "tests/corpus/reject/generic_bound.lc";
    let text = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(name))
        .unwrap();
    let result = runner::examine(name, &text);
    assert!(result.failures.is_empty(), "{:#?}", result.failures);
}

#[test]
fn comparison_inference_uses_boolean_result_not_operand_type() {
    run(
        "fn identity<T>(x: T) -> T { x } fn run() -> bool { let x: u8 = 2; let answer = identity(x < 3); answer }",
        "true",
    );
}

#[path = "common/compiled.rs"]
mod compiled;
#[test]
fn monomorphic_rust_compiles_with_warnings_denied_and_preserves_behavior() {
    let result = accepted(
        "fn identity<T>(x: T) -> T { x } fn run() -> u8 { let value: Option<u8> = Some(identity::<u8>(7)); match value { Option::Some(n) => n, Option::None => 0 } }",
    );
    let rust = locus::erased::print_module(result.session.erased());
    let source = compiled::harness(&[compiled::Unit {
        module: "example".into(),
        rust,
        calls: vec!["example::run()".into()],
    }]);
    for build in compiled::Overflow::ALL {
        let binary = compiled::compile("reconcile_generics", &source, build).unwrap();
        assert_eq!(
            compiled::observe(&binary, 1, std::time::Duration::from_secs(5)),
            vec![compiled::Answered::Value("7".into())]
        );
    }
}

#[test]
fn public_recursive_logical_functions_do_not_overflow_the_export_checker() {
    let source = "#[derive(Logical)] pub enum Peano { Zero, Succ(Peano) } pub logic fn size(n: Peano) -> Int { match n { Peano::Zero => 0, Peano::Succ(tail) => 1 + size(tail) } }";
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("recursive-export-{}.lc", std::process::id()));
    std::fs::write(&path, source).unwrap();
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_locus"));
    command.arg("check").arg(&path);
    for feature in [
        locus::preview::Feature::LogicalSplit,
        locus::preview::Feature::LogicalData,
    ] {
        if feature.status() == locus::preview::Status::Preview {
            command.args(["--preview", feature.name()]);
        }
    }
    let output = command.output().unwrap();
    let _ = std::fs::remove_file(path);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn recursive_logical_fields_still_expose_forgery_in_runtime_interfaces() {
    let prefix = "#[derive(Logical)] enum Claims { Again(Claims), End(@(false)) } ";
    let function = check(&format!(
        "{prefix} pub fn bad(claims: Claims) -> u8 {{ 0 }}"
    ));
    assert!(
        function.diagnostics.iter().any(|d| d.code == "L0244"),
        "{:#?}",
        function.diagnostics
    );
    let field = check(&format!("{prefix} pub struct Bad {{ pub claims: Claims }}"));
    assert!(
        field.diagnostics.iter().any(|d| d.code == "L0245"),
        "{:#?}",
        field.diagnostics
    );
}

#[test]
fn generic_logical_observers_check_each_concrete_physical_source() {
    accepted(
        "logic fn observe<T>(source: &T) -> Int { source as Int } fn observe_byte(source: u8) -> Int { observe::<u8>(&source) }",
    );
    accepted("logic fn unused<T>(source: &T) -> Int { source as Int }");
    let rejected = check(
        "struct Opaque { x: u8 } logic fn observe<T>(source: &T) -> Int { source as Int } fn bad(source: Opaque) -> Int { observe::<Opaque>(&source) }",
    );
    assert!(
        rejected.diagnostics.iter().any(|d| d.code == "L0282"),
        "{:#?}",
        rejected.diagnostics
    );
    let rejected = check("#[derive(Logical)] struct MissingBound<T> { value: T }");
    assert!(
        rejected.diagnostics.iter().any(|d| d.code == "L0281"),
        "{:#?}",
        rejected.diagnostics
    );
}

#[test]
fn associated_function_result_infers_generic_match_patterns() {
    accepted(
        "struct Percent { value: u8 }
        impl Percent {
            fn checked(value: u8) -> Option<Percent> {
                Option::Some(Percent { value })
            }
        }
        fn inspect(value: u8) -> u8 {
            match Percent::checked(value) {
                Option::Some(found) => found.value,
                Option::None => 0,
            }
        }",
    );
}
