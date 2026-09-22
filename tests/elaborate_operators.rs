//! The operators from source (build task E6): how the elaborator discharges
//! the obligation an operator carries under `no_panic`, by which tier, and
//! what the code after an operator knows, with and without the promise.
//! The corpus has the same programs run and compiled; this file looks at
//! the tiers and the diagnostics, which the corpus does not see. The
//! arithmetic tier of a hole (E7), and the interim rule for a function
//! that makes every promise of the logic and has an operator in its body
//! (LOC-193), are tested at the end.

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
        message
            .contains("it fails when n = 4294967295, which the arithmetic facts known here allow"),
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
    // In the body of a function that makes every promise of the logic the
    // operator stands, and the function is checked as an ordinary one with
    // its promises (LOC-193): here the obligation of `*` is what fails.
    let (codes, message) =
        rejected("#[terminates] #[no_panic] #[no_io]\nfn f(a: u8, b: u8) -> u8 {\n    a * b\n}\n");
    assert_eq!(codes, ["L0235"]);
    assert!(
        message.contains("`*` on `u8` may overflow, and `f` promises no_panic"),
        "{message}"
    );
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

// --- E7: arithmetic in holes and prove! ---------------------------------------------

#[test]
fn a_hole_is_tried_as_exact_computed_evaluation_then_arithmetic() {
    let result = accepted(
        "fn tiers(n: u8, small: @(n <= 10), bound: @(n as Int + 1 <= 11)) -> u8 {
    prove!(n <= 10);                  // exact
    let m = n;
    prove!(m <= 10);                  // computed: m stands for n
    prove!(3u8 <= 10);                // evaluation
    prove!(n as Int + 2 <= 12);       // arithmetic, from small
    prove!(n as Int + 1 <= 11);       // exact, although arithmetic would find it
    n
}
",
    );
    assert_eq!(
        tiers(&result),
        ["exact", "computed", "evaluation", "arithmetic", "exact"]
    );
}

#[test]
fn arithmetic_reads_the_facts_of_the_branch_taken_and_machine_equations() {
    // `lock.failures < 3` is known to the branch as the outcome of a
    // test; the tier reads it over the views by `cmp_reflect`, as it reads
    // an equation at a machine type by congruence of the view.
    let result = accepted(
        "fn bump(n: u32) -> u32 {
    if n < 3 {
        prove!(n as Int + 1 <= u32::MAX as Int);
        prove!(n as Int + 1 <= 3);
        n
    } else {
        prove!(3 <= n as Int);
        n
    }
}

fn same(a: u8, b: u8, h: @(a == b)) -> @(a as Int + 1 == b as Int + 1) {
    _
}
",
    );
    assert_eq!(
        tiers(&result),
        ["arithmetic", "arithmetic", "arithmetic", "arithmetic"]
    );
}

#[test]
fn a_goal_at_a_machine_type_is_bridged_to_the_views() {
    // An equation of machine values from one of their views, by the
    // injectivity of the view; a disequation as an implication whose
    // antecedent is assumed in both spellings.
    let result = accepted(
        "fn equal(a: u8, b: u8, h: @(a as Int == b as Int)) -> @(a == b) {
    _
}

fn apart(a: u8, b: u8, h: @(a as Int + 1 <= b as Int)) -> @(a != b) {
    _
}

fn below(a: u8, b: u8, h: @(a as Int <= b as Int - 1)) -> @(a < b) {
    _
}
",
    );
    assert_eq!(tiers(&result), ["arithmetic", "arithmetic", "arithmetic"]);
}

#[test]
fn the_midpoint_checks_and_runs() {
    let result = accepted(
        "#[terminates] #[no_panic] #[no_io]
pub fn midpoint(lo: u32, hi: u32, ordered: @(lo <= hi))
    -> (mid: u32, @(mid as Int == (lo as Int + hi as Int) / 2))
{
    let half = (hi - lo) / 2;
    let mid = lo + half;
    (mid, prove!(mid as Int == (lo as Int + hi as Int) / 2))
}
",
    );
    assert_eq!(
        tiers(&result),
        [
            "arithmetic",
            "arithmetic",
            "evaluation",
            "arithmetic",
            "arithmetic",
            "arithmetic"
        ]
    );
    let u32 = |value: i128| Value::Int(locus::kernel::MachineInt::U32, value);
    for (lo, hi, mid) in [
        (0, 0, 0),
        (0, 1, 0),
        (3, 10, 6),
        (4294967294, 4294967295, 4294967294),
    ] {
        for mode in Overflow::ALL {
            assert_eq!(
                run(
                    &result,
                    "midpoint",
                    vec![u32(lo), u32(hi), Value::Proved],
                    mode
                ),
                Outcome::Value(Value::Tuple(vec![u32(mid), Value::Proved]))
            );
        }
    }
}

#[test]
fn an_unsolved_hole_shows_the_counterexample_of_the_arithmetic_procedure() {
    let (codes, message) = rejected(
        "fn ordered(lo: u32, hi: u32) -> @(lo <= hi) {
    _
}
",
    );
    assert_eq!(codes, ["L0230"]);
    assert!(
        message
            .contains("it fails when lo = 1, hi = 0, which the arithmetic facts known here allow")
            || message.contains(
                "it fails when hi = 0, lo = 1, which the arithmetic facts known here allow"
            ),
        "{message}"
    );
    // The values satisfy the facts: not a point outside them.
    let (codes, message) = rejected(
        "fn nearly(n: u8, small: @(n <= 200)) -> @(n as Int + 100 <= 255) {
    _
}
",
    );
    assert_eq!(codes, ["L0230"]);
    assert!(
        message.contains("it fails when n = 156, which the arithmetic facts known here allow"),
        "{message}"
    );
    // When the point assigns a value to something the procedure does not
    // model, the view of a wrapped result, it is no counterexample and is
    // not shown; the note about the removed search stands instead.
    let (codes, message) = rejected(
        "fn wrapped(n: u8, small: @(n < 10)) -> @(n.wrapping_add(1) <= 10) {
    _
}
",
    );
    assert_eq!(codes, ["L0230"]);
    assert!(!message.contains("it fails when"), "{message}");
    assert!(
        message.contains("u8_succ_le_of_lt(n, 10, small)"),
        "{message}"
    );
}

#[test]
fn an_unsolved_hole_names_the_budget_that_ran_out() {
    // The problem of `tests/arith.rs` that blows up Fourier-Motzkin, as a
    // function of the logic over `Int`: ten unknowns, a bound on every
    // difference, and a false goal about all of them.
    let n = 10;
    let mut params: Vec<String> = (0..n).map(|i| format!("x{i}: Int")).collect();
    for i in 0..n {
        for j in 0..n {
            if i != j {
                params.push(format!("h{i}_{j}: @(x{i} - x{j} <= 1)"));
            }
        }
    }
    let sum = (1..n).fold("x0".to_string(), |acc, i| format!("{acc} + 2 * x{i}"));
    let text = format!(
        "#[terminates] #[no_panic] #[no_io]\nfn blown(\n    {}\n) -> @({sum} <= 0) {{\n    _\n}}\n",
        params.join(",\n    ")
    );
    let (codes, message) = rejected(&text);
    assert_eq!(codes, ["L0230"]);
    assert!(
        message.contains(
            "the arithmetic procedure ran out of its `derived` budget of 16384 before it could decide this"
        ),
        "{message}"
    );
}

#[test]
fn the_certificate_is_deterministic() {
    // The same file gives the same proofs, node for node: the procedure
    // is limited by counts, and the facts are presented in one order.
    let text = "#[no_panic]
fn scaled(n: u8, small: @(n <= 50)) -> (m: u8, @(m as Int <= 250)) {
    let m = n * 5;
    (m, prove!(m as Int <= 250))
}
";
    let first = accepted(text);
    let second = accepted(text);
    let proofs = |result: &Elaborated| -> Vec<(usize, &'static str)> {
        result
            .holes
            .iter()
            .map(|hole| (hole.proof_size, hole.tier))
            .collect()
    };
    assert_eq!(proofs(&first), proofs(&second));
    assert_eq!(tiers(&first), ["arithmetic", "arithmetic", "arithmetic"]);
}

// --- LOC-193: a fully promised function whose body is not a term ------------------

#[test]
fn a_fully_promised_function_with_an_operator_is_checked_as_an_ordinary_one() {
    // Every promise of the logic and `-` in the body: the obligation of
    // `-` is enforced, the function runs, and it cannot appear in a
    // proposition, since the kernel has no term for it.
    let result = accepted(
        "#[terminates] #[no_panic] #[no_io]
fn gap(hi: u8, lo: u8, ordered: @(lo <= hi)) -> u8 {
    hi - lo
}

fn uses_gap(hi: u8, lo: u8, ordered: @(lo <= hi)) -> u8 {
    gap(hi, lo, ordered)
}
",
    );
    assert_eq!(tiers(&result), ["arithmetic", "arithmetic"]);
    assert_eq!(
        run(
            &result,
            "uses_gap",
            vec![Value::u8(9), Value::u8(4), Value::Proved],
            Overflow::Checks
        ),
        Outcome::Value(Value::u8(5))
    );
    let (codes, message) = rejected(
        "#[terminates] #[no_panic] #[no_io]
fn gap(hi: u8, lo: u8, ordered: @(lo <= hi)) -> u8 {
    hi - lo
}

fn about(hi: u8, lo: u8, ordered: @(lo <= hi)) -> Prop {
    prop!(gap(hi, lo, ordered) <= hi)
}
",
    );
    assert_eq!(codes, ["L0209"]);
    assert_eq!(
        message,
        "`gap` cannot appear in a proposition: its body is not a term of the logic (it contains `-` at line 3); it is known by its contract only, which is not supported yet (LOC-193) a function appears in a proposition when it promises `terminates`, `no_panic`, and `no_io` and takes no `&mut`, so that mentioning it runs nothing and denotes one value"
    );
    // Without the bound the obligation fails, as in any `no_panic` function.
    let (codes, _) = rejected(
        "#[terminates] #[no_panic] #[no_io]\nfn gap(hi: u8, lo: u8) -> u8 {\n    hi - lo\n}\n",
    );
    assert_eq!(codes, ["L0235"]);
}

#[test]
fn a_fully_promised_function_without_an_operator_stays_a_function_of_the_logic() {
    // Its defining equation is known: the claim about it is `unfold!`ed,
    // and a value with a cast is a term.
    let result = accepted(
        "#[terminates] #[no_panic] #[no_io]
fn twice(n: u8) -> u8 {
    n.wrapping_add(n)
}

fn about(n: u8, h: @(twice(n) == 4)) -> @(n.wrapping_add(n) == 4) {
    unfold!(twice, h)
}

#[terminates] #[no_panic] #[no_io]
fn widened(n: u8) -> u16 {
    n as u16
}

fn wide(n: u8, h: @(widened(n) == 7)) -> @(n as u16 == 7) {
    unfold!(widened, h)
}
",
    );
    assert!(tiers(&result).is_empty());
}
