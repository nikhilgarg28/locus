//! The corpus: every `.lc` file under `examples`, `tests/corpus/accept`,
//! `tests/corpus/reject`, and `tests/corpus/target` is checked, run in both
//! interpreters, compiled, and compared, by the runner in
//! `tests/common/corpus.rs`, which explains the `//~` directives a file says
//! its expectations with. The runner is tested on itself below, with files
//! whose expectations are wrong.

#[path = "common/corpus.rs"]
mod runner;

use std::time::Duration;

use locus::elab::elaborate;
use locus::erased::{EBlock, EExpr, EStmt, Module, check_module};
use locus::kernel::MachineInt;
use locus::parser::parse;
use locus::source::SourceMap;
use locus::typed::PanicForm;
use runner::*;

#[test]
fn every_file_is_checked_run_in_both_interpreters_compiled_and_compared() {
    let mut failures = Vec::new();
    let mut inconclusive = Vec::new();
    let mut compiled = Vec::new();
    let mut examples = Vec::new();
    for (directory, rejects, target) in [
        ("examples", false, false),
        ("tests/corpus/accept", false, false),
        ("tests/corpus/reject", true, false),
        ("tests/corpus/target", false, true),
    ] {
        for (name, text) in files_in(directory) {
            let parse_only = is_parse_only(&text);
            if parse_only && !target {
                failures.push(Failure {
                    file: name.clone(),
                    line: 0,
                    message: "a file with a `parse-only` directive belongs in `target`".into(),
                });
            } else if !parse_only && expects_rejection(&text) != rejects {
                failures.push(Failure {
                    file: name.clone(),
                    line: 0,
                    message: if rejects {
                        "a file in `reject` needs an `error` directive".into()
                    } else {
                        "a file with an `error` directive belongs in `reject`".into()
                    },
                });
            }
            let examined = examine(&name, &text);
            failures.extend(examined.failures);
            inconclusive.extend(examined.inconclusive);
            compiled.extend(examined.compiled);
            if directory == "examples" {
                examples.push(name);
            }
        }
    }
    assert_eq!(
        examples,
        [
            "examples/increment.lc",
            "examples/lock.lc",
            "examples/optional_evidence.lc",
            "examples/optional_search.lc",
            "examples/preserve.lc",
            "examples/proofs.lc",
            "examples/propositions.lc",
        ]
    );
    let report = compile_and_compare(&compiled, "locus_corpus", TIMEOUT);
    failures.extend(report.failures);
    inconclusive.extend(report.inconclusive);
    print_inconclusive(&inconclusive);
    let listed: Vec<String> = failures.iter().map(Failure::to_string).collect();
    assert!(
        failures.is_empty(),
        "{} failure(s) in the corpus:\n{}",
        failures.len(),
        listed.join("\n")
    );
}

// The runner, tested on itself.

const INCREMENT: &str = "\
fn increment(n: u8) -> (out: u8, @(out == n.wrapping_add(1))) {
    (n.wrapping_add(1), _)
}
";

/// A failure of each interpreter in each mode, as the runner words them:
/// the interpreters first in one mode, then in the other.
fn in_each_interpreter(line: &str, rest: &str) -> Vec<String> {
    let mut messages = Vec::new();
    for build in Overflow::ALL {
        for interpreter in ["check-IR interpreter", "erased-tree interpreter"] {
            messages.push(format!("{line}: {interpreter}, {}: {rest}", build.name()));
        }
    }
    messages
}

/// The same for the erased-tree interpreter alone, as a tree built by hand
/// has no check IR.
fn in_erased_interpreter(line: &str, rest: &str) -> Vec<String> {
    Overflow::ALL
        .map(|build| format!("{line}: erased-tree interpreter, {}: {rest}", build.name()))
        .to_vec()
}

/// The failures of an in-memory file, as `line: message`.
fn failures_of(text: &str) -> Vec<String> {
    examine("memory.lc", text)
        .failures
        .iter()
        .map(|failure| format!("{}: {}", failure.line, failure.message))
        .collect()
}

#[test]
fn a_file_whose_expectations_hold_has_no_failures() {
    let text = format!(
        "{INCREMENT}//~ proofs: 1\n//~ run: increment(255) => (0, Erased)\n//~ rust: fn increment(n: u8) -> (u8, Erased) {{\n"
    );
    assert_eq!(failures_of(&text), Vec::<String>::new());
    let examined = examine("memory.lc", &text);
    let compiled = examined.compiled.expect("the file is accepted");
    assert_eq!(compiled.runs.len(), 1);
    assert_eq!(compiled.runs[0].call, "memory::increment(255)");
}

#[test]
fn a_wrong_expected_value_is_a_failure_in_each_interpreter() {
    let text = format!("{INCREMENT}//~ run: increment(1) => (3, Erased)\n");
    assert_eq!(
        failures_of(&text),
        in_each_interpreter(
            "4",
            "`increment(1)` is `(2, Erased)`, expected `(3, Erased)`"
        )
    );
}

const BUMP: &str = "\
fn bump(n: u8) -> u8 { n + 1 }
";

/// The corpus format can describe separate builds; Locus arithmetic itself
/// must give the checked result under either Rust setting.
#[test]
fn build_specific_expectations_do_not_change_checked_arithmetic() {
    let text = format!("{BUMP}//~ run: bump(255) => panic | 0\n");
    let expected: Vec<_> = ["check-IR interpreter", "erased-tree interpreter"].iter().map(|interpreter|
        format!("2: {interpreter}, overflow checks off: `bump(255)` is `panic: attempt to add with overflow`, expected `0`")).collect();
    assert_eq!(failures_of(&text), expected);
    let compiled = [examine("memory.lc", &text).compiled.unwrap()];
    assert_eq!(
        compiled[0].runs[0].expected_in(Overflow::Checked),
        &Expected::Panic(None)
    );
    assert_eq!(
        compiled[0].runs[0].expected_in(Overflow::Wrapping),
        &Expected::Value("0".into())
    );
    let observed = vec![Observed::of_line("panic: attempt to add with overflow")];
    assert!(
        compare(&compiled, Overflow::Checked, &observed)
            .failures
            .is_empty()
    );
    assert_eq!(
        compare(&compiled, Overflow::Wrapping, &observed)
            .failures
            .len(),
        1
    );
    assert!(failures_of(&format!("{BUMP}//~ run: bump(255) => panic\n")).is_empty());
}

#[test]
fn a_missing_error_is_a_failure() {
    let text = format!("{INCREMENT}//~^ error: L0230\n");
    assert_eq!(
        failures_of(&text),
        ["3: expected error L0230 on this line, and it was not reported"]
    );
}

#[test]
fn an_error_on_the_wrong_line_is_a_failure_twice() {
    let text = "\
fn first(n: u8) -> u8 { //~ error: L0204
    missing
}
";
    assert_eq!(
        failures_of(text),
        [
            "1: expected error L0204 on this line, and it was not reported",
            "2: unexpected error L0204: unknown name `missing`",
        ]
    );
    // On its line, with the right code, the file has no failures; with the
    // wrong code or the wrong message it has.
    let text = "fn first(n: u8) -> u8 {\n    missing //~ error: L0204 unknown name\n}\n";
    assert_eq!(failures_of(text), Vec::<String>::new());
    assert_eq!(failures_of(&text.replace("L0204", "L0220")).len(), 2);
    assert_eq!(
        failures_of(&text.replace("unknown name", "unknown type")),
        ["2: L0204 says `unknown name `missing``, which does not contain `unknown type`"]
    );
}

#[test]
fn an_error_in_a_file_that_expects_none_is_a_failure() {
    let text = "fn first(n: u8) -> u8 {\n    missing\n}\n//~ run: first(1) => 1\n";
    assert_eq!(
        failures_of(text),
        ["2: unexpected error L0204: unknown name `missing`"]
    );
    assert!(examine("memory.lc", text).compiled.is_none());
}

#[test]
fn every_failure_in_a_file_is_reported() {
    let text = format!(
        "{INCREMENT}//~ proofs: 2
//~ run: increment(1) => (2, Erased)
//~ run: increment(true) => (2, Erased)
//~ run: increment(1, 2) => (2, Erased)
//~ run: decrement(1) => 0
//~ run: increment(1) => panic
//~ run: increment(1)
//~ rust: fn decrement
//~ prooofs: 1
//~^ run: increment(1) => (2, Erased)
"
    );
    let mut expected = vec![
        "10: a run line reads `f(arguments) => value`".to_string(),
        "12: unknown directive `prooofs`; there are `proofs`, `run`, `rust`, `error`, `warning`, `preview`, `spec`, `known`, and `parse-only`".to_string(),
        "13: `^` belongs to `error` or `warning`, not `run`".to_string(),
        "4: 1 proof(s) were found, expected 2".to_string(),
        "6: `increment(true)`: expected a `u8`, found `true`".to_string(),
        "7: `increment(1, 2)`: more than 1 value(s) before `)`".to_string(),
        "8: `decrement(1)`: there is no `decrement` in the erased tree; a function that exists only in proofs cannot be run".to_string(),
    ];
    expected.extend(in_each_interpreter(
        "9",
        "`increment(1)` is `(2, Erased)`, expected `panic`",
    ));
    expected.push("11: the generated Rust does not contain `fn decrement`".to_string());
    assert_eq!(failures_of(&text), expected);
}

#[test]
fn a_rejected_file_is_not_run() {
    let text = "fn first(n: u8) -> u8 {\n    missing //~ error: L0204\n}\n//~ run: first(1) => 1\n";
    assert_eq!(
        failures_of(text),
        ["4: a file with an `error` directive is not run, so this expects nothing"]
    );
}

#[test]
fn arguments_are_read_by_type_and_written_as_rust() {
    let text = "\
enum Event { Wrong, Right(u8, bool) }
struct Lock { failures: u8, open: bool }
fn pick(lock: Lock, event: Event, pair: (u8, (bool,)), unit: ()) -> Event { event }
//~ run: pick(Lock { failures: 1, open: false }, Right(2, true), (3, (false,)), ()) => Right(2, true)
//~ run: pick(Lock { failures: 1, open: false, }, Event::Wrong, (3, (false,),), (),) => Wrong
//~ run: pick(Lock { open: false, failures: 1 }, Wrong, (3, (false,)), ()) => Wrong
//~ run: pick(Lock { failures: 1, open: false }, Lock::Wrong, (3, (false,)), ()) => Wrong
//~ run: pick(Lock { failures: 1, open: false }, Middle, (3, (false,)), ()) => Wrong
//~ run: pick(Lock { failures: 1, open: false }, Wrong, (3, (false,)), ()) extra => Wrong
";
    assert_eq!(
        failures_of(text),
        [
            "6: `pick(Lock { open: false, failures: 1 }, Wrong, (3, (false,)), ())`: expected field `failures`, found `open`",
            "7: `pick(Lock { failures: 1, open: false }, Lock::Wrong, (3, (false,)), ())`: expected a `Event`, found `Lock`",
            "8: `pick(Lock { failures: 1, open: false }, Middle, (3, (false,)), ())`: `Event` has no variant `Middle`",
            "9: `pick(Lock { failures: 1, open: false }, Wrong, (3, (false,)), ()) extra`: unexpected `extra` after the call",
        ]
    );
    let compiled = examine("memory.lc", text).compiled.unwrap();
    assert_eq!(
        compiled.runs[0].call,
        "memory::pick(memory::Lock { failures: 1, open: false }, memory::Event::Right(2, true), (3, (false,)), ())"
    );
}

#[test]
fn compiled_output_that_differs_is_a_failure_on_its_run_line() {
    let text = format!(
        "{INCREMENT}//~ run: increment(1) => (2, Erased)\n//~ run: increment(2) => (3, Erased)\n"
    );
    let compiled = [examine("memory.lc", &text).compiled.unwrap()];
    let failures = |output: &str| -> Vec<String> {
        let observed: Vec<Seen> = output.lines().map(Observed::of_line).collect();
        let report = compare(&compiled, Overflow::Checked, &observed);
        assert!(report.inconclusive.is_empty());
        report.failures.iter().map(Failure::to_string).collect()
    };
    assert_eq!(failures("(2, Erased)\n(3, Erased)\n"), Vec::<String>::new());
    assert_eq!(
        failures("(2, Erased)\n(4, Erased)\n"),
        [
            "memory.lc:5: compiled Rust, overflow checks on: `memory::increment(2)` is `(4, Erased)`, expected `(3, Erased)`"
        ]
    );
    assert_eq!(
        failures("(2, Erased)\npanic: attempt to add with overflow\n"),
        [
            "memory.lc:5: compiled Rust, overflow checks on: `memory::increment(2)` is `panic: attempt to add with overflow`, expected `(3, Erased)`"
        ]
    );
    assert_eq!(
        failures("(2, Erased)\n"),
        ["memory.lc:5: compiled Rust, overflow checks on: `memory::increment(2)` was not answered"]
    );
    assert_eq!(
        failures("(2, Erased)\n(3, Erased)\n7\n"),
        ["the compiled program: overflow checks on: 1 answer(s) more than there are run lines"]
    );
    // The harness is one program: a module for the file, and the calls, each
    // under `catch_unwind`.
    let source = harness(&compiled);
    assert!(
        source.contains("mod memory {\n// Generated by Locus."),
        "{source}"
    );
    assert!(source.contains("    crate::answer(1, from, || memory::increment(2));\n"));
    assert!(source.contains("std::panic::catch_unwind(call)"));
    assert!(!source.contains("answer_lending("));
    // A batch with no run lines is a program all the same.
    assert!(harness(&[]).ends_with("fn main() {}\n"));
}

// Panics and returns. No source returns yet, and the forms that panic came
// with E10 (tests/corpus/accept/panic_forms.lc); the harness itself is tested
// on erased trees given them by hand: every call of `panics(k)` becomes a
// `panic!` with message `k`, every call of `returns(e)` a `return e`, and
// every call of `returns_pair(k)` a `return (k, k)`.

const PANICS: &str = "\
enum Event { Wrong, Right(u8) }

fn panics(which: u8) -> u8 { which }

fn second(a: u8, b: u8) -> u8 { b }

fn in_let(n: u8) -> u8 {
    let m = panics(0);
    m
}

fn in_argument(n: u8) -> u8 { second(n, panics(1)) }

fn first_argument_first(n: u8) -> u8 { second(panics(2), panics(1)) }

fn in_tuple(n: u8) -> (u8, u8, u8) { (n, panics(3), n.wrapping_add(1)) }

fn in_arm(event: Event) -> u8 {
    match event {
        Event::Wrong => panics(4),
        Event::Right(n) => n,
    }
}

fn after_three(n: u8) -> u8 {
    let mut i: u8 = 0;
    loop {
        if i == n {
            break i
        } else {
            let _ = if i == 3 { panics(5) } else { i };
            i = i.wrapping_add(1);
        }
    }
}

fn at_the_top(n: u8) -> u8 {
    if n == 255 { panics(6) } else { n.wrapping_add(1) }
}

fn through_a_call(n: u8) -> u8 { second(at_the_top(n), 7) }
";

const MESSAGES: [&str; 7] = [
    "in a let",
    "in an argument",
    "the first argument",
    "in a tuple",
    "say \"hi\" to {n}, }{ and {{ and \\ too",
    "two\nlines",
    "f(255)",
];

const PANICS_RUNS: &str = r#"
//~ run: in_let(1) => panic: in a let
//~ run: in_argument(1) => panic: in an argument
//~ run: first_argument_first(1) => panic: the first argument
//~ run: in_tuple(1) => panic: in a tuple
//~ run: in_arm(Wrong) => panic: say "hi" to {n}, }{ and {{ and \\ too
//~ run: in_arm(Right(9)) => 9
//~ run: after_three(3) => 3
//~ run: after_three(4) => panic: two\nlines
//~ run: at_the_top(254) => 255
//~ run: at_the_top(255) => panic
//~ run: at_the_top(255) => panic: f(255)
//~ run: through_a_call(255) => panic: f(255)
//~ run: through_a_call(1) => 7
//~ rust: Event::Wrong => {
//~ rust: panic!("{}", "say \"hi\" to {n}, }{ and {{ and \\ too")
//~ rust: panic!("{}", "two\nlines")
"#;

/// The erased tree of `PANICS`, with its panics.
fn panicking_module() -> Module {
    planted_module(PANICS)
}

/// The erased tree of a source, with its panics and returns planted.
fn planted_module(text: &str) -> Module {
    let mut sources = SourceMap::default();
    let file = sources.add("planted.lc", text);
    let source = sources.get(file);
    let elaborated = elaborate(source, &parse(source).program);
    assert!(elaborated.is_success(), "{:?}", elaborated.diagnostics);
    let mut module = elaborated.session.erased().clone();
    for function in &mut module.fns {
        plant_in_block(&mut function.body);
    }
    assert_eq!(check_module(&module), Ok(()));
    module
}

fn plant_in_block(block: &mut EBlock) {
    for stmt in &mut block.stmts {
        match stmt {
            EStmt::Let { value, .. } | EStmt::Assign { value, .. } => plant(value),
            EStmt::Expr(expr) => plant(expr),
        }
    }
    if let Some(tail) = &mut block.tail {
        plant(tail);
    }
}

fn plant(expr: &mut EExpr) {
    match expr {
        EExpr::Call {
            name, arguments, ..
        } if name == "returns" => {
            let [value] = arguments.as_mut_slice() else {
                panic!("`returns` takes one argument")
            };
            plant(value);
            *expr = EExpr::Return(Box::new(value.clone()));
        }
        EExpr::Call {
            name, arguments, ..
        } if name == "panics" || name == "returns_pair" => {
            let [EExpr::Literal(MachineInt::U8, which)] = arguments.as_slice() else {
                panic!("`{name}` takes a literal")
            };
            let which = *which;
            let index = usize::try_from(which).expect("a small byte");
            *expr = if name == "panics" {
                EExpr::Panic {
                    form: PanicForm::Panic,
                    argument: Some(MESSAGES[index].into()),
                }
            } else {
                EExpr::Return(Box::new(EExpr::Tuple(vec![
                    EExpr::Literal(MachineInt::U8, which),
                    EExpr::Literal(MachineInt::U8, which),
                ])))
            };
        }
        EExpr::Var { .. }
        | EExpr::Bool(_)
        | EExpr::Literal(..)
        | EExpr::Proved
        | EExpr::Ghost
        | EExpr::Trap
        | EExpr::Lend { .. }
        | EExpr::Panic { .. } => {}
        EExpr::Buffer {
            arguments: exprs, ..
        }
        | EExpr::Tuple(exprs)
        | EExpr::Variant { payload: exprs, .. }
        | EExpr::Call {
            arguments: exprs, ..
        }
        | EExpr::Operate {
            operands: exprs, ..
        } => exprs.iter_mut().for_each(plant),
        EExpr::Continue => {}
        EExpr::Break(inner) => inner.iter_mut().for_each(|inner| plant(inner)),
        EExpr::Struct { fields, .. } => fields.iter_mut().for_each(|(_, value)| plant(value)),
        EExpr::BoxNew(inner)
        | EExpr::BoxDeref(inner)
        | EExpr::Shared { value: inner, .. }
        | EExpr::Deref(inner)
        | EExpr::Field { target: inner, .. }
        | EExpr::Cast { expr: inner, .. }
        | EExpr::Assert {
            condition: inner, ..
        }
        | EExpr::Return(inner) => {
            plant(inner);
        }
        EExpr::Method {
            receiver,
            arguments,
            ..
        } => {
            plant(receiver);
            arguments.iter_mut().for_each(plant);
        }
        EExpr::Compare { left, right, .. } => {
            plant(left);
            plant(right);
        }
        EExpr::If {
            condition,
            then_block,
            else_block,
        } => {
            plant(condition);
            plant_in_block(then_block);
            plant_in_block(else_block);
        }
        EExpr::Match {
            scrutinee, arms, ..
        } => {
            plant(scrutinee);
            arms.iter_mut()
                .for_each(|arm| plant_in_block(&mut arm.body));
        }
        EExpr::Block(block) | EExpr::Loop { body: block, .. } => plant_in_block(block),
        EExpr::While { condition, body } => {
            plant(condition);
            plant_in_block(body);
        }
        EExpr::For { lo, hi, body, .. } => {
            plant(lo);
            plant(hi);
            plant_in_block(body);
        }
    }
}

fn listed(remarks: &[Failure]) -> Vec<String> {
    remarks.iter().map(Failure::to_string).collect()
}

const RETURNS: &str = "\
enum Event { Wrong, Right(u8) }

fn returns(which: u8) -> u8 { which }

fn returns_pair(which: u8) -> (u8, u8) { (which, which) }

fn panics(which: u8) -> u8 { which }

fn after_a_let(n: u8) -> u8 {
    let m = panics(0);
    m.wrapping_add(1)
}

fn early(n: u8) -> u8 {
    let m = if n == 0 { returns(7) } else { n };
    m.wrapping_add(1)
}

fn from_a_loop(n: u8) -> u8 {
    let mut i: u8 = 0;
    loop {
        if i == n {
            break returns(i.wrapping_add(100))
        } else {
            i = i.wrapping_add(1);
        }
    }
}

fn from_a_for(n: u8) -> u8 {
    let mut last: u8 = 0;
    for i in 0..n {
        let _ = if i == 2 { returns(50) } else { i };
        last = i;
    }
    last
}

fn from_an_arm(event: Event) -> u8 {
    let (value, _) = match event {
        Event::Wrong => (returns(4), 0u8),
        Event::Right(n) => (n, n),
    };
    value
}

fn with_a_wildcard(n: u8) -> (u8, u8) {
    let (a, _) = if n == 0 { returns_pair(6) } else { returns_pair(8) };
    (a, 0)
}

fn through_a_call(n: u8) -> u8 { early(n).wrapping_add(10) }
";

const RETURNS_RUNS: &str = "\
//~ run: after_a_let(1) => panic: in a let
//~ run: early(0) => 7
//~ run: early(5) => 6
//~ run: from_a_loop(0) => 100
//~ run: from_a_loop(3) => 103
//~ run: from_a_for(2) => 1
//~ run: from_a_for(5) => 50
//~ run: from_an_arm(Wrong) => 4
//~ run: from_an_arm(Right(9)) => 9
//~ run: with_a_wildcard(0) => (6, 6)
//~ run: with_a_wildcard(1) => (8, 8)
//~ run: through_a_call(0) => 17
//~ run: through_a_call(5) => 16
//~ rust: panic!(\"{}\", \"in a let\")
//~ rust: let m: u8 = if n == 0_u8 {
//~ rust: return 7_u8
//~ rust: break (return i.wrapping_add(100_u8))
//~ rust: let _ = if i == 2_u8 {
//~ rust: return 50_u8
//~ rust: let (value, _): (u8, _) = match event {
//~ rust: Event::Wrong => {
//~ rust: ((return 4_u8), 0_u8)
//~ rust: if n == 0_u8 {
//~ rust: return (6_u8, 6_u8)
";

#[test]
fn trees_that_return_agree_with_their_compiled_rust_and_a_let_bound_to_a_panic_has_a_type() {
    let module = planted_module(RETURNS);
    let text = format!("{RETURNS}{RETURNS_RUNS}");
    let examined = examine_tree("returns.lc", &text, &module, FUEL);
    assert_eq!(listed(&examined.failures), Vec::<String>::new());
    assert_eq!(listed(&examined.inconclusive), Vec::<String>::new());
    let report = compile_and_compare(
        &[examined.compiled.unwrap()],
        "locus_corpus_returns",
        TIMEOUT,
    );
    assert_eq!(listed(&report.failures), Vec::<String>::new());
    assert_eq!(listed(&report.inconclusive), Vec::<String>::new());
}

#[test]
fn trees_that_panic_agree_with_their_compiled_rust_message_included() {
    let module = panicking_module();
    let text = format!("{PANICS}{PANICS_RUNS}");
    let right = examine_tree("panics.lc", &text, &module, FUEL);
    assert_eq!(listed(&right.failures), Vec::<String>::new());
    assert_eq!(listed(&right.inconclusive), Vec::<String>::new());
    // The braces of a message are not the printer's: what follows the
    // message with more `{` than `}` is indented as it would be without it.
    let rust = &right.compiled.as_ref().unwrap().rust;
    assert!(
        rust.contains("pub fn after_three(n: u8) -> u8 {\n    let mut i = 0_u8;\n    loop {\n"),
        "{rust}"
    );

    // The same tree with expectations that are wrong in each way there is:
    // a panic where it returns, another message, and a value where it
    // panics. The interpreter says so, and so does each build.
    let wrong_runs = "\
//~ run: at_the_top(1) => panic
//~ run: at_the_top(255) => panic: f(254)
//~ run: at_the_top(255) => 0
//~ run: at_the_top(255) => panic: f(255)
";
    let wrong = examine_tree("wrong.lc", wrong_runs, &module, FUEL);
    assert_eq!(
        listed(&wrong.failures),
        [
            in_erased_interpreter("wrong.lc:1", "`at_the_top(1)` is `2`, expected `panic`"),
            in_erased_interpreter(
                "wrong.lc:2",
                "`at_the_top(255)` is `panic: f(255)`, expected `panic: f(254)`"
            ),
            in_erased_interpreter(
                "wrong.lc:3",
                "`at_the_top(255)` is `panic: f(255)`, expected `0`"
            ),
        ]
        .concat()
    );

    let compiled = [right.compiled.unwrap(), wrong.compiled.unwrap()];
    let report = compile_and_compare(&compiled, "locus_corpus_panics", TIMEOUT);
    assert_eq!(listed(&report.inconclusive), Vec::<String>::new());
    let expected: Vec<String> = ["overflow checks on", "overflow checks off"]
        .iter()
        .flat_map(|build| {
            [
                format!(
                    "wrong.lc:1: compiled Rust, {build}: `wrong::at_the_top(1)` is `2`, expected `panic`"
                ),
                format!(
                    "wrong.lc:2: compiled Rust, {build}: `wrong::at_the_top(255)` is `panic: f(255)`, expected `panic: f(254)`"
                ),
                format!(
                    "wrong.lc:3: compiled Rust, {build}: `wrong::at_the_top(255)` is `panic: f(255)`, expected `0`"
                ),
            ]
        })
        .collect();
    assert_eq!(listed(&report.failures), expected);
}

#[test]
fn out_of_fuel_is_inconclusive_and_is_not_a_panic() {
    // `after_three(200)` panics in its fourth iteration, and with fuel for
    // fewer it has done neither that nor anything else. Whatever the run
    // line expects, the answer is that there is none. A call that needs less
    // fuel is still compared.
    let module = panicking_module();
    let runs = "\
//~ run: after_three(200) => panic
//~ run: after_three(200) => panic: two\\nlines
//~ run: after_three(200) => 200
//~ run: at_the_top(1) => 2
";
    let examined = examine_tree("fuel.lc", runs, &module, 25);
    assert_eq!(listed(&examined.failures), Vec::<String>::new());
    let no_answer = "`after_three(200)` gave no answer: out of fuel after 25 steps";
    assert_eq!(
        listed(&examined.inconclusive),
        [
            in_erased_interpreter("fuel.lc:1", no_answer),
            in_erased_interpreter("fuel.lc:2", no_answer),
            in_erased_interpreter("fuel.lc:3", no_answer),
        ]
        .concat()
    );
    // With fuel, the first two hold and the third is wrong.
    let examined = examine_tree("fuel.lc", runs, &module, FUEL);
    assert_eq!(
        listed(&examined.failures),
        in_erased_interpreter(
            "fuel.lc:3",
            "`after_three(200)` is `panic: two\\nlines`, expected `200`"
        )
    );
    assert!(examined.inconclusive.is_empty());
}

#[test]
fn a_run_line_that_never_answers_is_inconclusive_and_the_rest_are_compared() {
    let text = "\
fn forever(n: u8) -> u8 {
    let mut i: u8 = n;
    loop {
        i = i.wrapping_add(1);
    }
}

fn next(n: u8) -> u8 { n.wrapping_add(1) }

//~ run: next(1) => 2
//~ run: forever(1) => 7
//~ run: next(2) => 3
//~ run: forever(2) => panic
//~ run: next(3) => 9
";
    // In the interpreters, out of fuel: inconclusive, whatever was expected.
    let (found, _) = directives_of("forever.lc", text);
    let mut sources = SourceMap::default();
    let file = sources.add("forever.lc", text);
    let source = sources.get(file);
    let elaborated = elaborate(source, &parse(source).program);
    assert!(elaborated.is_success(), "{:?}", elaborated.diagnostics);
    let subject = Subject {
        module: elaborated.session.erased(),
        program: Some(elaborated.session.program()),
        lending: Some(elaborated.session.lending()),
        proofs: 0,
        visibilities: elaborated.visibilities.clone(),
    };
    let examined = examine_accepted("forever.lc", found, &subject, 10_000);
    let by_interpreter = |line: usize, call: &str| {
        in_each_interpreter(
            &format!("forever.lc:{line}"),
            &format!("`{call}` gave no answer: out of fuel after 10000 steps"),
        )
    };
    assert_eq!(
        listed(&examined.inconclusive),
        [
            by_interpreter(11, "forever(1)"),
            by_interpreter(13, "forever(2)")
        ]
        .concat()
    );
    assert_eq!(
        listed(&examined.failures),
        in_each_interpreter("forever.lc:14", "`next(3)` is `4`, expected `9`")
    );

    // Compiled, each `forever` is killed at the timeout, which is short
    // here. It is inconclusive, and the run lines after it are compared: the
    // last one fails, in both builds.
    let timeout = Duration::from_secs(2);
    let report = compile_and_compare(
        &[examined.compiled.unwrap()],
        "locus_corpus_forever",
        timeout,
    );
    let builds = ["overflow checks on", "overflow checks off"];
    let killed = |build: &str, line: usize, call: &str| {
        format!(
            "forever.lc:{line}: compiled Rust, {build}: `{call}` gave no answer: killed after 2s without an answer"
        )
    };
    assert_eq!(
        listed(&report.inconclusive),
        builds
            .map(|build| {
                [
                    killed(build, 11, "forever::forever(1)"),
                    killed(build, 13, "forever::forever(2)"),
                ]
            })
            .concat()
    );
    assert_eq!(
        listed(&report.failures),
        builds.map(|build| format!(
            "forever.lc:14: compiled Rust, {build}: `forever::next(3)` is `4`, expected `9`"
        ))
    );
}
