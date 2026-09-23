//! Logical callable values, dependent results, and capture boundaries.
use locus::elab::{Elaborated, Options, elaborate_with_options};
use locus::erased::check_module;
use locus::parser::parse;
use locus::source::SourceMap;
fn check(text: &str) -> Elaborated {
    let mut map = SourceMap::default();
    let id = map.add("closures.lc", text);
    let source = map.get(id);
    let parsed = parse(source);
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
    let mut options = Options::default();
    for name in ["logical-split", "logical-data", "named-props"] {
        let feature = locus::preview::Feature::parse(name).unwrap();
        if feature.status() == locus::preview::Status::Preview {
            options.previews.enable(name).unwrap();
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
#[test]
fn predicate_closure_passes_to_a_logic_function() {
    accepted(
        "logic fn apply(p: logic Fn(x: Int) -> Prop, value: Int) -> Prop { p(value) }
        logic fn sample() -> Prop { apply(|x: Int| prop!(x >= 0), 3) }",
    );
}
#[test]
fn proof_function_result_depends_on_its_argument() {
    accepted(
        "logic fn at_three(p: logic Fn(x: Int) -> @(x == x)) -> @(3 == 3) { p(3) }
        logic fn sample() -> @(3 == 3) { at_three(|n: Int| prove!(n == n)) }",
    );
}
#[test]
#[doc = "spec: 1.25:3"]
fn immutable_logical_captures_and_nested_closures_check() {
    accepted("logic fn make(n: Int) -> logic Fn(x: Int) -> Int { |x: Int| n + x }
        logic fn nested(n: Int) -> logic Fn(x: Int) -> logic Fn(y: Int) -> Int { |x: Int| |y: Int| n + x + y }
        logic fn sample() -> Int { let add = make(3); add(4) }");
}
#[test]
#[doc = "spec: 1.25:3"]
fn runtime_capture_requires_a_model_and_logical_parameters_are_required() {
    accepted("fn make(n: u8) -> logic Fn(x: Int) -> Int { logic { |x: Int| x + (n as Int) } }");
    for source in [
        "fn make(n: u8) -> logic Fn(x: Int) -> Int { logic { |x: Int| x + n } }",
        "logic fn bad() -> logic Fn(x: Int) -> Int { |x: u8| x as Int }",
    ] {
        let result = check(source);
        assert!(
            result.diagnostics.iter().any(|d| d.code == "L0283"),
            "{:#?}",
            result.diagnostics
        );
    }
}
#[test]
fn closure_cannot_call_runtime_code_or_return_runtime_data() {
    for source in [
        "fn effect(x: u8) -> Int { 0 } logic fn bad() -> logic Fn(x: Int) -> Int { |x: Int| effect(0) }",
        "struct Runtime { n: u8 } fn bad() -> () { let f = logic { |x: Int| Runtime { n: 0 } }; }",
    ] {
        let result = check(source);
        assert!(!result.is_success(), "{source}");
    }
}
#[test]
fn beta_reduction_supports_proving_concrete_applications() {
    accepted(
        "logic fn sample() -> @(3 == 3) { let identity = |x: Int| x; let h = prove!(identity(3) == 3); prove!(3 == 3) }",
    );
}

#[test]
fn generic_sequence_map_uses_a_capturing_logical_closure() {
    accepted(&format!(
        "{}\nlogic fn mapped() -> Seq<Int> {{ let increment: Int = 2; let xs: Seq<Int> = Seq::Cons {{ head: 1, tail: Seq::Empty }}; seq_map(xs, |n: Int| n + increment) }}",
        include_str!("../library/logical.lc")
    ));
}

#[test]
fn logical_application_in_runtime_code_retains_callee_and_argument_effects_once() {
    let result = accepted(
        "fn make(n: &mut u8) -> logic Fn(x: Int) -> Int { n = n.wrapping_add(1); |x: Int| x }
        fn argument(n: &mut u8) -> u8 { n = n.wrapping_add(2); n }
        fn run() -> u8 { let mut n: u8 = 0; let erased = make(&mut n)(argument(&mut n)); n }",
    );
    let module = result.session.erased();
    let value = locus::erased::Interpreter::new(module, 10000)
        .call(result.function("run").unwrap(), vec![])
        .unwrap();
    assert_eq!(value.debug(module), "3");
}

#[test]
fn a_logical_callable_type_cannot_hide_runtime_inputs_or_outputs() {
    for source in [
        "fn bad(f: logic Fn(x: Int) -> u8) -> u8 { f(0) }",
        "fn bad(f: logic Fn(x: u8) -> Int) -> Int { f(0u8) }",
        "fn bad(f: fn(u8) -> u8) -> u8 { f(0u8) }",
    ] {
        let result = check(source);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "L0270"),
            "{:#?}",
            result.diagnostics
        );
    }
}
