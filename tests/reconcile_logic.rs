//! The logical split is checked independently of the output cleanup.
use locus::elab::{Elaborated, Options, elaborate_with_options};
use locus::erased::{Interpreter, Value, check_module};
use locus::parser::parse;
use locus::source::SourceMap;

fn check(text: &str) -> Elaborated {
    let mut map = SourceMap::default();
    let id = map.add("logical-split.lc", text);
    let source = map.get(id);
    let parsed = parse(source);
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let mut options = Options::default();
    if locus::preview::Feature::parse("logical-split")
        .unwrap()
        .status()
        == locus::preview::Status::Preview
    {
        options.previews.enable("logical-split").unwrap();
    }
    if locus::preview::Feature::parse("named-props")
        .unwrap()
        .status()
        == locus::preview::Status::Preview
    {
        options.previews.enable("named-props").unwrap();
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
#[doc = "spec: 1.3:1, 1.5:4"]
fn logical_bool_is_not_a_runtime_condition_or_result() {
    for text in [
        "fn wrong(b: Bool) -> u8 { if b { 1 } else { 0 } }",
        "fn wrong(b: Bool) -> bool { b }",
        "fn wrong(b: Bool) -> bool { return b; }",
        "fn consume(b: bool) -> bool { b } fn wrong(b: Bool) -> bool { consume(b) }",
        "fn wrong(b: Bool) -> u8 { let c: bool = b; 0 }",
        "struct Runtime { b: bool } fn wrong(b: Bool) -> Runtime { Runtime { b } }",
    ] {
        let result = check(text);
        assert!(
            result.diagnostics.iter().any(|d| d.code == "L0272"),
            "accepted or wrong diagnostic: {text}\n{:#?}",
            result.diagnostics
        );
    }
}
#[test]
#[doc = "spec: 1.5:2, 1.5:3"]
fn ordinary_promises_never_make_a_logic_function() {
    let result = check(
        "#[terminates] #[no_panic] #[no_io] fn identity(x: u32) -> u32 { x } fn wrong(x: u32) -> Prop { prop!(identity(x) == x) }",
    );
    assert!(
        result.diagnostics.iter().any(|d| d.code == "L0209"),
        "{:#?}",
        result.diagnostics
    );
}
#[test]
#[doc = "spec: 1.5:4, 1.6:3"]
fn explicit_logic_definitions_compute_and_erase() {
    let result = accepted(
        "logic fn positive(n: Int) -> Bool { n > 0 } fn example(n: u32) -> u32 { let p = positive(n); let q: Bool = logic { 2 > 1 }; n }",
    );
    let module = result.session.erased();
    let function = result.function("example").unwrap();
    let value = Interpreter::new(module, 1000)
        .call(
            function,
            vec![Value::Int(locus::kernel::MachineInt::U32, 9)],
        )
        .unwrap();
    assert_eq!(value.debug(module), "9");
    assert!(!module.fns.iter().any(|f| f.name == "positive"));
}
#[test]
fn comparisons_are_kernel_checked_via_holds() {
    accepted(
        "fn ordered(n: u32) -> @(n >= 0) { prove!(n >= 0) } fn sample() -> @(3 < 4) { prove!(3 < 4) }",
    );
}
#[test]
fn erased_observation_keeps_mutation() {
    let result = accepted(
        "fn bump(x: &mut u32) -> u32 { x = x.wrapping_add(1); x } logic fn identity(n: Int) -> Int { n } fn main() -> u32 { let mut n: u32 = 2; let ignored = identity(bump(&mut n)); n }",
    );
    let module = result.session.erased();
    let value = Interpreter::new(module, 1000)
        .call(result.function("main").unwrap(), vec![])
        .unwrap();
    assert_eq!(value.debug(module), "3");
}
#[test]
fn logical_boolean_operators_can_appear_in_ordinary_code() {
    accepted("fn combine(a: Bool, b: Bool) -> Bool { let c = a && b; let d = !c; d || b }");
    accepted("logic fn combine(a: Bool, b: Bool) -> Bool { a && (!b || a) }");
}
#[test]
#[doc = "spec: 1.15:3, 1.6:2"]
fn default_model_and_explicit_model_state_the_same_claim() {
    accepted("fn same(n: u32) -> @(prop!(n <= 3) == prop!((n as Int) <= 3)) { _ }");
    accepted("fn same(b: bool) -> @(prop!(b) == prop!(b as Bool)) { _ }");
}
#[test]
#[doc = "spec: 1.18:2, 1.5:1"]
#[doc = "spec: 3.3:1, 3.5:2"]
fn logical_calls_keep_argument_effect_order_and_calls_returning_only_proofs() {
    let result = accepted(
        "fn bump(x: &mut u32) -> @(true) { x = x.wrapping_add(1); True::Intro } logic fn consume(p: @(true)) -> @(true) { p } fn main() -> u32 { let mut n: u32 = 0; let proof = consume(bump(&mut n)); n }",
    );
    let module = result.session.erased();
    assert_eq!(
        Interpreter::new(module, 1000)
            .call(result.function("main").unwrap(), vec![])
            .unwrap()
            .debug(module),
        "1"
    );
}
#[test]
#[doc = "spec: 1.5:3"]
fn logic_blocks_reject_runtime_calls_even_when_the_result_is_logical() {
    let result = check(
        "fn evidence() -> @(true) { True::Intro } fn wrong() -> @(true) { logic { evidence() } }",
    );
    assert!(
        result.diagnostics.iter().any(|d| d.code == "L0209"),
        "{:#?}",
        result.diagnostics
    );
}
#[test]
#[doc = "spec: 1.18:1, 1.5:5"]
fn mixed_tuples_preserve_each_fields_logical_mode() {
    for text in [
        "fn leak(t: (Bool, u8)) -> bool { t.0 }",
        "fn leak(t: ((u8, Bool), u8)) -> bool { t.0.1 }",
        "fn leak(t: (Bool, u8)) -> (bool, u8) { t }",
        "fn leak(t: (Bool, u8)) -> bool { let (b, n) = t; b }",
    ] {
        let result = check(text);
        assert!(
            result.diagnostics.iter().any(|d| d.code == "L0272"),
            "{text}\n{:#?}",
            result.diagnostics
        );
    }
    accepted("fn keep(t: (Bool, u8)) -> (Bool, u8) { let copy = t; copy }");
    accepted(
        "fn make(n: u8) -> (Bool, u8) { (true, n) } fn keep(n: u8) -> u8 { let (b, x) = make(n); let c: Bool = b; x }",
    );
    accepted(
        "struct Packet { pair: (Bool, u8) } fn make(n: u8) -> Packet { Packet { pair: (true, n) } } fn get(p: Packet) -> Bool { p.pair.0 }",
    );
}
#[test]
fn logical_results_do_not_change_runtime_branch_effects() {
    let result = accepted(
        "fn hit(n: &mut u32) -> Bool { n = n.wrapping_add(1); true } fn choose(c: bool) -> u32 { let mut n: u32 = 0; let b: Bool = if c { hit(&mut n) } else { false }; n }",
    );
    let module = result.session.erased();
    for (flag, expected) in [(false, "0"), (true, "1")] {
        assert_eq!(
            Interpreter::new(module, 1000)
                .call(result.function("choose").unwrap(), vec![Value::Bool(flag)])
                .unwrap()
                .debug(module),
            expected
        );
    }
    let result = accepted(
        "fn hit(n: &mut u32) -> Bool { n = n.wrapping_add(1); true } fn choose() -> u32 { let mut n: u32 = 0; let a: Bool = false; let b = a && hit(&mut n); n }",
    );
    let module = result.session.erased();
    assert_eq!(
        Interpreter::new(module, 1000)
            .call(result.function("choose").unwrap(), vec![])
            .unwrap()
            .debug(module),
        "1"
    );
}

#[test]
fn logical_tuple_layout_survives_loops_and_enum_patterns() {
    accepted("fn keep(b: Bool) -> (Bool, u8) { loop { break (b, 7); } }");
    accepted(
        "enum E { Value((Bool, u8)) } fn keep(e: E) -> (Bool, u8) { match e { E::Value(pair) => pair } }",
    );
    for text in [
        "fn leak(b: Bool) -> bool { let x = loop { break b; }; x }",
        "enum E { Value((Bool, u8)) } fn leak(e: E) -> bool { match e { E::Value(pair) => pair.0 } }",
        "fn bad(b: Bool) -> () { while b { break; } }",
        "fn bad(b: Bool) -> bool { let mut x = false; x = b; x }",
        "fn read(x: &bool) -> () {} fn bad(b: Bool) -> Bool { read(&b); b }",
    ] {
        let result = check(text);
        assert!(
            result.diagnostics.iter().any(|d| d.code == "L0272"),
            "{text}\n{:?}",
            result.diagnostics
        );
    }
}
