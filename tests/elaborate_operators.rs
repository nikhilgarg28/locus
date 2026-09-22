//! The operators from source (build task E6): how the elaborator discharges
//! the obligation an operator carries under `no_panic`, by which tier, and
//! what the code after an operator knows, with and without the promise.
//! The corpus has the same programs run and compiled; this file looks at
//! the tiers and the diagnostics, which the corpus does not see.

use locus::elab::{Elaborated, elaborate};
use locus::erased::{Interpreter, Outcome, Overflow, Value, check_module};
use locus::parser::parse;
use locus::source::SourceMap;

const FUEL: u64 = 100_000;

fn elaborated(text: &str) -> Elaborated {
    let mut sources = SourceMap::default();
    let file = sources.add("test.lc", text);
    let source = sources.get(file);
    let parsed = parse(source);
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
    elaborate(source, &parsed.program)
}

fn accepted(text: &str) -> Elaborated {
    let result = elaborated(text);
    let messages: Vec<String> = result
        .diagnostics
        .iter()
        .map(|d| format!("{}: {} {:?}", d.code, d.message, d.notes))
        .collect();
    assert!(result.is_success(), "{messages:#?}");
    assert_eq!(check_module(result.session.erased()), Ok(()));
    result
}

/// The codes of the diagnostics, and the first one's full text.
fn rejected(text: &str) -> (Vec<&'static str>, String) {
    let result = elaborated(text);
    assert!(!result.is_success(), "accepted:\n{text}");
    let first = &result.diagnostics[0];
    let full = format!("{} {}", first.message, first.notes.join(" "));
    (result.diagnostics.iter().map(|d| d.code).collect(), full)
}

fn tiers(result: &Elaborated) -> Vec<&'static str> {
    result.holes.iter().map(|hole| hole.tier).collect()
}

fn run(result: &Elaborated, name: &str, arguments: Vec<Value>, mode: Overflow) -> Outcome {
    let module = result.session.erased();
    let function = result.function(name).expect("the function exists");
    Interpreter::new(module, FUEL)
        .with_overflow(mode)
        .call(function, arguments)
        .expect("the call runs")
}

#[test]
fn an_obligation_is_discharged_by_a_fact_in_scope_and_the_exact_result_follows() {
    // Case 7 of the design: `fits` gives the upper bound exactly, once the
    // view of the literal `1` is computed on the premise's side; the lower
    // bound is the arithmetic procedure's, from the range of `n`.
    let result = accepted(
        "#[no_panic]
fn next(n: u32, fits: @(n as Int + 1 <= u32::MAX as Int)) -> (m: u32, @(m as Int == n as Int + 1)) {
    let m = n + 1;
    (m, _)
}
",
    );
    assert_eq!(tiers(&result), ["arithmetic", "exact", "computed"]);
    assert_eq!(
        run(
            &result,
            "next",
            vec![
                Value::Int(locus::kernel::MachineInt::U32, 41),
                Value::Proved
            ],
            Overflow::Checks
        ),
        Outcome::Value(Value::Tuple(vec![
            Value::Int(locus::kernel::MachineInt::U32, 42),
            Value::Proved
        ]))
    );
}

#[test]
fn an_obligation_is_discharged_by_a_prove_on_the_line_before() {
    // The `prove!` states the premise where the operator needs it, and is
    // itself exact from the parameter; the operator's obligation is then
    // the exact tier, and the exact sum is known after.
    let result = accepted(
        "#[no_panic]
fn ten_more(n: u8, room: @(n as Int + 10 <= 255)) -> (m: u8, @(m as Int == n as Int + 10)) {
    prove!(n as Int + 10 <= 255);
    let m = n + 10;
    (m, _)
}
",
    );
    assert_eq!(tiers(&result), ["exact", "arithmetic", "exact", "computed"]);
}

#[test]
fn an_obligation_is_discharged_by_arithmetic_from_a_bound_on_the_operand() {
    let result = accepted(
        "#[no_panic]
fn scaled(n: u8, small: @(n <= 50)) -> u8 {
    n * 5
}
",
    );
    assert_eq!(tiers(&result), ["arithmetic", "arithmetic"]);
    // A chain: the second operator's obligation follows from the first's
    // exact result.
    let result = accepted(
        "#[no_panic]
fn twice_then_one(n: u8, small: @(n <= 100)) -> u8 {
    let d = n * 2;
    d + 1
}
",
    );
    assert_eq!(
        tiers(&result),
        ["arithmetic", "arithmetic", "arithmetic", "arithmetic"]
    );
}

#[test]
fn an_obligation_on_literals_is_decided_by_evaluation() {
    let result = accepted(
        "#[no_panic]
fn hundred_and_ten() -> u8 {
    100 + 10
}
",
    );
    assert_eq!(tiers(&result), ["evaluation", "evaluation"]);
    assert_eq!(
        run(&result, "hundred_and_ten", vec![], Overflow::Checks),
        Outcome::Value(Value::u8(110))
    );
}

#[test]
fn an_undischarged_obligation_is_reported_at_the_operator_with_its_premise() {
    let (codes, message) = rejected(
        "#[no_panic]
fn bump(n: u32) -> u32 {
    n + 1
}
",
    );
    assert_eq!(codes, ["L0235"]);
    assert!(
        message.contains("`+` on `u32` may overflow, and `bump` promises no_panic"),
        "{message}"
    );
    assert!(
        message.contains("the sum `n as Int + 1` must be at most 4294967295"),
        "{message}"
    );
    assert!(
        message.contains("a counterexample: n = 4294967295"),
        "{message}"
    );
    assert!(
        message.contains("`prove!(n as Int + 1 <= 4294967295);` just before this"),
        "{message}"
    );
    // The same body without the promise is accepted: no obligation.
    let result = accepted("fn bump(n: u32) -> u32 {\n    n + 1\n}\n");
    assert!(tiers(&result).is_empty());
}

#[test]
fn the_exact_result_is_known_under_no_panic_and_not_without_it() {
    let text = |promise: &str| {
        format!(
            "{promise}
fn sum(a: u8, b: u8, fits: @(a as Int + b as Int <= 255)) -> u8 {{
    let s = a + b;
    prove!(s as Int == a as Int + b as Int);
    s
}}
"
        )
    };
    let result = accepted(&text("#[no_panic]"));
    assert_eq!(tiers(&result), ["arithmetic", "exact", "computed"]);
    let (codes, message) = rejected(&text(""));
    assert_eq!(codes, ["L0230"]);
    assert!(message.contains("cannot show `s == a + b`"), "{message}");
}

#[test]
fn the_wrapped_result_is_known_in_every_function() {
    let result = accepted(
        "fn sum(a: u8, b: u8) -> (s: u8, @(s == (a as Int + b as Int) as u8)) {
    let s = a + b;
    (s, _)
}
",
    );
    assert_eq!(tiers(&result), ["computed"]);
    // And it holds in both builds: the wrapping build gives the wrapped
    // value, the checked build panics rather than give another.
    let arguments = vec![Value::u8(200), Value::u8(100)];
    assert_eq!(
        run(&result, "sum", arguments.clone(), Overflow::Wrap),
        Outcome::Value(Value::Tuple(vec![Value::u8(44), Value::Proved]))
    );
    assert_eq!(
        run(&result, "sum", arguments, Overflow::Checks),
        Outcome::Panic("attempt to add with overflow".into())
    );
}

#[test]
fn a_division_teaches_its_condition_without_the_promise() {
    let result = accepted(
        "fn quotient(a: u8, b: u8) -> (q: u8, @(b != 0)) {
    let q = a / b;
    (q, _)
}

fn remainder(a: i64, b: i64) -> (r: i64, @(b as Int != 0)) {
    let r = a % b;
    (r, _)
}
",
    );
    assert_eq!(tiers(&result), ["exact", "exact"]);
    for mode in Overflow::ALL {
        assert_eq!(
            run(&result, "quotient", vec![Value::u8(7), Value::u8(0)], mode),
            Outcome::Panic("attempt to divide by zero".into())
        );
    }
    // Under the promise the condition is the obligation, met here by the
    // machine-typed fact, and at a signed type both conditions are.
    let result = accepted(
        "#[no_panic]
fn safe(a: u8, b: u8, nonzero: @(b != 0)) -> u8 {
    a / b
}

#[no_panic]
fn safe_signed(a: i8, b: i8, positive: @(0 < b)) -> i8 {
    a % b
}
",
    );
    assert_eq!(tiers(&result), ["computed", "arithmetic", "arithmetic"]);
    let (codes, message) = rejected(
        "#[no_panic]
fn halve(a: u8, b: u8) -> u8 {
    a / b
}
",
    );
    assert_eq!(codes, ["L0235"]);
    assert!(
        message.contains("the divisor `b` must not be zero"),
        "{message}"
    );
}

#[test]
fn an_operator_on_a_machine_type_is_refused_where_nothing_runs() {
    let (codes, message) = rejected("fn p(a: u8, b: u8) -> Prop {\n    prop!(a + b <= 255)\n}\n");
    assert_eq!(codes, ["L0236"]);
    assert_eq!(
        message,
        "`+` on `u8` may panic, on overflow, so it is not a proposition write `a as Int + b as Int` for the exact sum, or `a.wrapping_add(b)` for the wrapped one"
    );
    let (codes, _) =
        rejected("#[terminates] #[no_panic] #[no_io]\nfn f(a: u8, b: u8) -> u8 {\n    a * b\n}\n");
    assert_eq!(codes, ["L0236"]);
    // On `Int` the operators are total, and stand in a claim.
    let result = accepted(
        "fn commutes(a: u8, b: u8, h: @(a as Int + b as Int <= 255)) -> @(a as Int + b as Int <= 255) {
    h
}

fn negated(a: i8, h: @(-(a as Int) <= 128)) -> @(-(a as Int) <= 128) {
    h
}
",
    );
    assert!(tiers(&result).is_empty());
}

#[test]
fn the_operands_are_typed_as_a_comparisons_are() {
    let (codes, message) = rejected("fn f(a: u8, b: u32) -> u32 {\n    a + b\n}\n");
    assert_eq!(codes, ["L0211"]);
    assert!(
        message.contains("`+` is between two values of one type"),
        "{message}"
    );
    let (codes, _) = rejected("fn f(a: u8) -> u8 {\n    -a\n}\n");
    assert_eq!(codes, ["L0237"]);
    let (codes, _) = rejected("fn f(a: bool, b: bool) -> bool {\n    a + b\n}\n");
    assert_eq!(codes, ["L0238"]);
    // Two literals take the type expected of them; a literal beside a value
    // takes the value's type.
    let result = accepted(
        "fn literals() -> u16 {\n    let x: u16 = 1 + 2;\n    x * 3\n}\nfn beside(a: i64) -> i64 {\n    a - 1\n}\n",
    );
    assert_eq!(
        run(&result, "literals", vec![], Overflow::Checks),
        Outcome::Value(Value::Int(locus::kernel::MachineInt::U16, 9))
    );
}
