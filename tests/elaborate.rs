//! Source to checked program: parsing, elaboration with holes, kernel
//! acceptance, interpretation, and generated Rust, from `.lc` text.

use std::path::PathBuf;
use std::process::Command;

use locus::elab::{Elaborated, elaborate};
use locus::erased::{Interpreter, Outcome, Value, check_module, print_module};
use locus::parser::parse;
use locus::source::SourceMap;

const FUEL: u64 = 1_000_000;
const LOCK: &str = include_str!("../examples/lock.lc");

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

fn call(result: &Elaborated, name: &str, arguments: &[u8]) -> String {
    let module = result.session.erased();
    let function = result.function(name).expect("the function exists");
    let values = arguments.iter().copied().map(Value::u8).collect();
    Interpreter::new(module, FUEL)
        .call(function, values)
        .expect("the call returns")
        .debug(module)
}

#[test]
fn the_acceptance_examples_check() {
    for example in [
        include_str!("../examples/increment.lc"),
        include_str!("../examples/preserve.lc"),
        include_str!("../examples/proofs.lc"),
        include_str!("../examples/propositions.lc"),
        LOCK,
    ] {
        accepted(example);
    }
}

/// What `examples/lock.lc` computes, written directly.
fn attempts_left(attempts: u8, correct: u8) -> u8 {
    let mut failures = 0u8;
    for attempt in 0..attempts {
        failures = if attempt == correct {
            0
        } else if failures < 3 {
            failures + 1
        } else {
            failures
        };
    }
    3 - failures
}

#[test]
fn the_fix_for_the_retired_math_fn_yields_a_file_that_checks_with_the_same_proofs() {
    // Every `math fn` in the file is reported with a fix; the fixes applied,
    // last first, give a file that parses, elaborates, and proves what the
    // hand-written promises prove: one hole, filled the same way.
    let retired = include_str!("corpus/reject/math_fn_keyword.lc");
    let mut sources = SourceMap::default();
    let file = sources.add("math_fn_keyword.lc", retired);
    let parsed = parse(sources.get(file));
    assert!(!parsed.is_success());
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == "L0114"),
        "{:?}",
        parsed.diagnostics
    );
    let mut fixes: Vec<_> = parsed
        .diagnostics
        .iter()
        .flat_map(|diagnostic| diagnostic.suggestions.iter())
        .collect();
    assert_eq!(fixes.len(), 3);
    fixes.sort_by_key(|fix| std::cmp::Reverse(fix.span.start));
    let mut fixed = retired.to_owned();
    for fix in fixes {
        fixed.replace_range(fix.span.range(), &fix.replacement);
    }
    let by_hand = retired
        .replace("pub math fn ", "#[terminates] #[no_panic] #[no_io] pub fn ")
        .replace(
            "pub(crate) math fn ",
            "#[terminates] #[no_panic] #[no_io] pub(crate) fn ",
        )
        .replace("\nmath fn ", "\n#[terminates] #[no_panic] #[no_io] fn ");
    let fixed = accepted(&fixed);
    let by_hand = accepted(&by_hand);
    let proofs = |result: &Elaborated| -> Vec<(usize, &str, usize)> {
        result
            .holes
            .iter()
            .map(|hole| (hole.span.start, hole.tier, hole.proof_size))
            .collect()
    };
    assert_eq!(proofs(&fixed), proofs(&by_hand));
    assert_eq!(fixed.holes.len(), 1);
    assert!(fixed.holes.iter().all(|hole| hole.solved));
    assert_eq!(call(&fixed, "twice_of", &[4]), "8");
    assert_eq!(call(&fixed, "math", &[4]), "4");
    assert_eq!(
        print_module(fixed.session.erased()),
        print_module(by_hand.session.erased())
    );
}

#[test]
fn the_lock_runs_as_written() {
    let result = accepted(LOCK);
    for attempts in [0, 1, 2, 3, 4, 5, 9, 200, 255] {
        for correct in [0, 1, 3, 8, 254] {
            assert_eq!(
                call(&result, "attempts_left", &[attempts, correct]),
                attempts_left(attempts, correct).to_string(),
                "attempts_left({attempts}, {correct})"
            );
        }
    }
    // Every `_` and `prove!` was filled and accepted by the kernel.
    assert!(result.holes.iter().all(|hole| hole.solved));
    // The proofs of `lock.lc`, by position, tier, and size, asserted
    // exactly on purpose: a hole is filled by a fact that matches, after
    // computing, or by evaluation, and nothing else, so a change in what
    // is found, or in how large it is, is a change to review.
    let mut sources = SourceMap::default();
    let file = sources.add("lock.lc", LOCK);
    let source = sources.get(file);
    let found: Vec<(usize, usize, &str, usize)> = result
        .holes
        .iter()
        .map(|hole| {
            let (line, column) = source.line_column(hole.span.start).unwrap();
            (line, column, hole.tier, hole.proof_size)
        })
        .collect();
    assert_eq!(
        found,
        [
            // step: `prove!(lock.failures < 3)` is the branch taken, reflected
            // through `cmp_reflect`, whose comparison names its type.
            (34, 63, "computed", 14),
            // step: `bounded` serves for `within_limit((Lock { .. }).failures)`.
            (37, 65, "computed", 37),
            // step: `prove!(0u8 <= 3)`: the order of two views, evaluated as
            // it stands.
            (31, 80, "evaluation", 11),
            // run: `prove!(0u8 <= 3)` for the initial `ok`.
            (56, 68, "evaluation", 11),
            // run: `ok = still` refreshes the tracked evidence over the
            // `lock` just assigned: `still` speaks of `next`, and `lock`
            // is `next` after `lock = next`, which computing bridges.
            (60, 14, "computed", 52),
        ]
    );
}

#[test]
fn the_generated_rust_reads_like_the_source_and_agrees_with_the_interpreter() {
    let result = accepted(LOCK);
    let module = result.session.erased();
    let mut source = print_module(module);
    for expected in [
        "#[derive(Clone, Copy, Debug)]\npub struct Lock {",
        "pub fn step(lock: Lock, bounded: Proved, event: Event) -> (Lock, Proved) {",
        "    match event {",
        "            if lock.failures < 3_u8 {",
        "                (Lock { failures: lock.failures.wrapping_add(1_u8), open: false }, Proved)",
        "    let mut ok = Proved;",
        "    for attempt in 0_u8..attempts {",
        "        let (next, still) = step(lock, ok, event_at(attempt, correct));",
        "        lock = next;",
        "        ok = Proved;",
        "    (3_u8.wrapping_sub(failures), Proved)",
        "    let (last, bounded) = run(attempts, correct);",
    ] {
        assert!(source.contains(expected), "missing: {expected}\n{source}");
    }

    let inputs: Vec<(u8, u8)> = vec![(0, 0), (1, 0), (2, 5), (3, 5), (4, 5), (9, 5), (255, 7)];
    let mut expected_output = String::new();
    source.push_str("\nfn main() {\n");
    for (attempts, correct) in &inputs {
        source.push_str(&format!(
            "    println!(\"{{:?}} {{:?}}\", run({attempts}, {correct}), attempts_left({attempts}, {correct}));\n"
        ));
        expected_output.push_str(&format!(
            "{} {}\n",
            call(&result, "run", &[*attempts, *correct]),
            call(&result, "attempts_left", &[*attempts, *correct])
        ));
    }
    source.push_str("}\n");

    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let source_path = directory.join("locus_lock.rs");
    let binary_path = directory.join("locus_lock");
    std::fs::write(&source_path, &source).unwrap();
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let compile = Command::new(rustc)
        .args(["--edition", "2021", "-D", "warnings", "-o"])
        .arg(&binary_path)
        .arg(&source_path)
        .output()
        .expect("rustc runs");
    assert!(
        compile.status.success(),
        "rustc rejected the generated code:\n{}\n{source}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let run = Command::new(&binary_path)
        .output()
        .expect("the program runs");
    assert!(run.status.success());
    assert_eq!(String::from_utf8_lossy(&run.stdout), expected_output);
}

#[test]
fn a_missing_guard_is_reported_with_the_claim_the_facts_and_a_failing_case() {
    let guarded = "            if lock.failures < 3 {
                let fits = u8_succ_le_of_lt(lock.failures, 3, prove!(lock.failures < 3));
                (Lock { failures: lock.failures.wrapping_add(1), open: false }, fold!(within_limit, fits))
            } else {
                (Lock { failures: lock.failures, open: false }, bounded)
            }";
    assert!(LOCK.contains(guarded));
    let unguarded = LOCK.replace(
        guarded,
        "            (Lock { failures: lock.failures.wrapping_add(1), open: false }, _)",
    );
    let result = elaborated(&unguarded);
    // One error: what depends on `step` is not reported again.
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    let error = &result.diagnostics[0];
    assert_eq!(error.code, "L0230");
    assert_eq!(
        error.message,
        "cannot show `within_limit(lock.failures.wrapping_add(1))`"
    );
    assert_eq!(
        error.notes,
        [
            "known here: `bounded: within_limit(lock.failures)`",
            "it fails when `lock.failures` is 3, which the facts known here allow",
        ]
    );
    // Nothing unverified was emitted.
    assert!(result.function("step").is_none());
    assert!(result.function("run").is_none());
    assert!(result.function("event_at").is_some());
}

// Routine rearrangements of a program that checks need no proof repair.

const INCREMENT: &str = "fn increment(n: u8) -> (out: u8, @(out == n.wrapping_add(1))) {
    let out = n.wrapping_add(1);
    (out, _)
}";

#[test]
fn introducing_a_local_needs_no_proof_repair() {
    let result = accepted(
        "fn increment(n: u8) -> (out: u8, @(out == n.wrapping_add(1))) {
            let one: u8 = 1;
            let sum = n.wrapping_add(one);
            let out = sum;
            (out, _)
        }",
    );
    assert_eq!(call(&result, "increment", &[255]), "(0, Proved)");
    let inline = accepted(
        "fn increment(n: u8) -> (out: u8, @(out == n.wrapping_add(1))) { (n.wrapping_add(1), _) }",
    );
    assert_eq!(call(&inline, "increment", &[7]), "(8, Proved)");
}

#[test]
fn destructuring_a_result_keeps_its_evidence_usable() {
    // `first_is` is an equation about `first`, and `rewrite!` carries
    // `second_is` across it; the result is the claim stated, exactly.
    let result = accepted(&format!(
        "{INCREMENT}
        fn twice(n: u8) -> (out: u8, @(out == n.wrapping_add(1).wrapping_add(1))) {{
            let (first, first_is) = increment(n);
            let (second, second_is) = increment(first);
            (second, rewrite!(first_is, second_is))
        }}
        fn twice_without_names(n: u8) -> (out: u8, @(out == n.wrapping_add(1).wrapping_add(1))) {{
            let once = increment(n);
            let again = increment(once.0);
            (again.0, rewrite!(once.1, again.1))
        }}"
    ));
    assert_eq!(call(&result, "twice", &[254]), "(0, Proved)");
    assert_eq!(call(&result, "twice_without_names", &[1]), "(3, Proved)");
    // Without the step, the equation in scope rewrites nothing by itself,
    // and the diagnostic names the step.
    let (codes, full) = rejected(&format!(
        "{INCREMENT}
        fn twice(n: u8) -> (out: u8, @(out == n.wrapping_add(1).wrapping_add(1))) {{
            let (first, first_is) = increment(n);
            let (second, second_is) = increment(first);
            (second, _)
        }}"
    ));
    assert_eq!(codes, ["L0230"]);
    assert!(
        full.contains("this follows from `second_is` by `rewrite!(first_is, second_is)`"),
        "{full}"
    );
}

#[test]
fn a_dependent_pattern_opens_over_its_own_names() {
    // Finding 2 of the target examples: in `let (next, still) = bump(..)`,
    // `still` is evidence about `next`, not about `bump(..).0`, so it is
    // exactly what the result asks for and no hole is filled at all.
    let result = accepted(
        "fn bump(n: u8, small: @(n < 10)) -> (out: u8, @(out <= 10)) {
            (n.wrapping_add(1), u8_succ_le_of_lt(n, 10, small))
        }
        fn use_it(n: u8, small: @(n < 10)) -> (out: u8, @(out <= 10)) {
            let (next, still) = bump(n, small);
            (next, still)
        }",
    );
    assert!(result.holes.is_empty(), "{:#?}", result.holes);
    let module = result.session.erased();
    let value = Interpreter::new(module, FUEL)
        .call(
            result.function("use_it").unwrap(),
            vec![Value::u8(9), Value::Proved],
        )
        .unwrap();
    assert_eq!(value.debug(module), "(10, Proved)");
    // The type of `still` speaks of `next`, as a mismatch shows. (`n <=
    // 10` itself would be filled from `small` by the arithmetic tier.)
    let (codes, full) = rejected(
        "fn bump(n: u8, small: @(n < 10)) -> (out: u8, @(out <= 10)) {
            (n.wrapping_add(1), u8_succ_le_of_lt(n, 10, small))
        }
        fn use_it(n: u8, small: @(n < 10)) -> @(n <= 5) {
            let (next, still) = bump(n, small);
            still
        }",
    );
    assert_eq!(codes, ["L0230"]);
    assert!(
        full.starts_with("this is evidence of `next <= 10`, and `n <= 5` is needed"),
        "{full}"
    );
    // A later part may speak of two earlier names, and a name that is not
    // data leaves no equation behind.
    let result = accepted(
        "fn pair(n: u8) -> (a: u8, b: u8, @(a <= b)) {
            (n, n, u8_le_refl(n))
        }
        fn use_pair(n: u8) -> (a: u8, b: u8, @(a <= b)) {
            let (x, y, ordered) = pair(n);
            (x, y, ordered)
        }",
    );
    assert!(result.holes.is_empty(), "{:#?}", result.holes);
    assert_eq!(call(&result, "use_pair", &[4]), "(4, 4, Proved)");
}

#[test]
fn extracting_a_helper_needs_no_proof_repair() {
    // `step` of the lock, with the saturating increment moved into a helper
    // whose result says what the caller needs.
    let result = accepted(
        "#[terminates] #[no_panic] #[no_io] fn within_limit(failures: u8) -> Prop { prop!(failures <= 3) }
        fn bump(failures: u8, bounded: @within_limit(failures)) -> (next: u8, @within_limit(next)) {
            if failures < 3 {
                (failures.wrapping_add(1), fold!(within_limit, u8_succ_le_of_lt(failures, 3, prove!(failures < 3))))
            } else {
                (failures, bounded)
            }
        }
        fn record(failures: u8, bounded: @within_limit(failures), wrong: u8) -> (next: u8, @within_limit(next)) {
            if wrong == 0 {
                (0, fold!(within_limit, prove!(0u8 <= 3)))
            } else {
                let (next, still) = bump(failures, bounded);
                (next, still)
            }
        }",
    );
    // Erasure keeps the parameter list: evidence is passed as a marker.
    let module = result.session.erased();
    let value = Interpreter::new(module, FUEL)
        .call(
            result.function("record").unwrap(),
            vec![Value::u8(3), Value::Proved, Value::u8(1)],
        )
        .unwrap();
    assert_eq!(value.debug(module), "(3, Proved)");
}

#[test]
fn evidence_about_a_name_serves_for_a_name_bound_to_it() {
    // `still` is evidence about `next`; `copy` is bound to `next`, and
    // computing replaces the name, so the evidence serves.
    let result = accepted(
        "fn bump(n: u8, small: @(n < 10)) -> (out: u8, @(out <= 10)) {
            (n.wrapping_add(1), u8_succ_le_of_lt(n, 10, small))
        }
        fn use_it(n: u8, small: @(n < 10)) -> (out: u8, @(out <= 10)) {
            let (next, still) = bump(n, small);
            let copy = next;
            (copy, still)
        }",
    );
    let tiers: Vec<&str> = result.holes.iter().map(|hole| hole.tier).collect();
    assert_eq!(tiers, ["computed"]);
}

#[test]
fn a_function_of_the_logic_runs_and_is_usable_in_claims() {
    let result = accepted(
        "#[terminates] #[no_panic] #[no_io] fn double_step(n: u8) -> u8 { n.wrapping_add(1).wrapping_add(1) }
        fn advance(n: u8) -> (out: u8, @(out == double_step(n))) {
            let out = n.wrapping_add(1).wrapping_add(1);
            (out, fold!(double_step, prove!(out == n.wrapping_add(1).wrapping_add(1))))
        }",
    );
    assert_eq!(call(&result, "double_step", &[254]), "0");
    assert_eq!(call(&result, "advance", &[3]), "(5, Proved)");
}

#[test]
fn a_loop_supplies_its_value_and_evidence_at_the_break() {
    let result = accepted(
        "fn walk(limit: u8) -> (out: u8, @(out <= limit)) {
            let mut i: u8 = 0;
            loop {
                if i == limit {
                    break (limit, u8_le_refl(limit))
                } else {
                    i = i.wrapping_add(1);
                }
            }
        }",
    );
    assert_eq!(call(&result, "walk", &[9]), "(9, Proved)");
    // The evidence is checked against the versions current at the break,
    // and nothing carried from earlier passes speaks of `i` there. (The
    // branch's `i == limit` would give `i <= limit` by arithmetic, so the
    // claim is one the branch does not decide.)
    let (codes, full) = rejected(
        "fn walk(limit: u8) -> (out: u8, @(out <= 5)) {
            let mut i: u8 = 0;
            loop {
                if i == limit {
                    break (i, prove!(i <= 5))
                } else {
                    i = i.wrapping_add(1);
                }
            }
        }",
    );
    assert_eq!(codes, ["L0230"]);
    assert!(full.contains("cannot show `i <= 5`"), "{full}");
}

#[test]
fn a_loop_carries_what_its_body_assigns_and_the_rest_is_read_after_it() {
    let result = accepted(
        "fn tally(n: u8) -> (u8, u8) {
            let mut evens: u8 = 0;
            let mut odds: u8 = 0;
            let mut k: u8 = 0;
            while k < n {
                if k.wrapping_mul(128) == 128 { odds = odds.wrapping_add(1); } else { evens = evens.wrapping_add(1); }
                k = k.wrapping_add(1);
            }
            (evens, odds)
        }",
    );
    assert_eq!(call(&result, "tally", &[7]), "(4, 3)");
    // What a loop assigned is all that is known of it afterwards.
    let (codes, full) = rejected(
        "fn f(n: u8) -> u8 {
            let mut x: u8 = 0;
            for i in 0..n { x = i; }
            prove!(x == 0);
            x
        }",
    );
    assert_eq!(codes, ["L0230"]);
    assert!(full.contains("cannot show `x == 0`"), "{full}");
}

#[test]
fn boolean_connectives_short_circuit_and_each_test_feeds_its_branch() {
    // `3 <= n && n <= 9` is an `if`, whose branch knows the whole test and
    // not its parts; a test whose outcome a branch needs is its own `if`.
    let result = accepted(
        "fn clamp(n: u8) -> (out: u8, @(out <= 9)) {
            if 3 <= n { if n <= 9 { (n, _) } else { (9, _) } } else { (9, _) }
        }
        fn either(n: u8) -> bool { n == 0 || !(n < 200) }",
    );
    assert_eq!(call(&result, "clamp", &[5]), "(5, Proved)");
    assert_eq!(call(&result, "clamp", &[77]), "(9, Proved)");
    assert_eq!(call(&result, "either", &[0]), "true");
    assert_eq!(call(&result, "either", &[100]), "false");
    assert_eq!(call(&result, "either", &[250]), "true");
}

#[test]
fn a_wildcard_arm_covers_the_remaining_variants() {
    let result = accepted(
        "enum Light { Red, Amber(u8), Green }
        fn wait(light: Light) -> u8 {
            match light {
                Light::Amber(seconds) => seconds,
                _ => 0,
            }
        }
        fn probe(n: u8) -> u8 { wait(Light::Amber(n)) }",
    );
    assert_eq!(call(&result, "probe", &[4]), "4");
}

#[test]
fn errors_name_the_problem() {
    let cases: [(&str, &str, &str); 15] = [
        (
            "fn f() -> u8 { missing }",
            "L0204",
            "unknown name `missing`",
        ),
        ("fn f() -> u8 { 256 }", "L0205", "does not fit in `u8`"),
        (
            "fn f() -> u8 { true }",
            "L0220",
            "expected `u8`, found `bool`",
        ),
        ("fn f(n: Nat) -> u8 { 0 }", "L0201", "not part of the core"),
        (
            "fn f(n: u8) -> u8 { let x = _; n }",
            "L0206",
            "nothing here says of what",
        ),
        (
            "fn f(n: u8) -> u8 { f(n) }",
            "L0203",
            "defined in terms of itself",
        ),
        (
            "fn f(n: u8) -> u8 { n } #[terminates] #[no_panic] #[no_io] fn g(n: u8) -> u8 { f(n) }",
            "L0232",
            "`g` promises terminates and calls `f`, which does not",
        ),
        (
            "enum E { A, B } fn f(e: E) -> u8 { match e { E::A => 0 } }",
            "L0213",
            "no arm handles `E::B`",
        ),
        (
            "fn f(n: u8) -> u8 { let mut a: u8 = 0; for i in 0..n { a = i; break a } a }",
            "L0218",
            "`break` with a value leaves a `while` or a `for`",
        ),
        (
            "fn f(n: u8) -> u8 { loop (i: u8 = 0) -> u8 { break i } }",
            "L0234",
            "was removed; write `loop { ... }`",
        ),
        (
            "#[terminates] #[no_panic] #[no_io] fn f(n: u8) -> u8 { loop { break n } }",
            "L0215",
            "`loop` cannot appear in `f`, which promises terminates",
        ),
        (
            "fn f(n: u8) -> u8 { n.pow(2) }",
            "L0207",
            "unknown method `pow`",
        ),
        (
            "fn f(n: u8) -> u8 { n } fn g() -> u8 { f(1, 2) }",
            "L0208",
            "takes 1 value, and 2 were given",
        ),
        (
            "fn f(flag: bool) -> @(flag) { _ }",
            "L0221",
            "this is a `bool`, and a proposition is needed",
        ),
        (
            "fn f() -> u8 { 1 } fn f() -> u8 { 2 }",
            "L0202",
            "declared twice",
        ),
    ];
    for (text, code, fragment) in cases {
        let (codes, full) = rejected(text);
        assert_eq!(codes, [code], "{text}: {full}");
        assert!(full.contains(fragment), "{text}: {full}");
    }
}

#[test]
fn a_false_claim_is_refuted_with_a_case() {
    let (codes, full) = rejected("fn f(n: u8) -> @(n.wrapping_add(1) != 0) { _ }");
    assert_eq!(codes, ["L0230"]);
    assert!(full.contains("it fails when `n` is 255"), "{full}");
    let (_, full) = rejected("fn f(n: u8, m: u8) -> @(n <= m) { _ }");
    assert!(full.contains("cannot show `n <= m`"), "{full}");
    assert!(full.contains("nothing known here"), "{full}");
}

#[test]
fn a_reversed_range_runs_no_pass_and_needs_no_evidence() {
    let result = accepted(
        "fn f(n: u8) -> u8 {
            let mut passes: u8 = 0;
            for i in 5..n { passes = passes.wrapping_add(1); }
            passes
        }",
    );
    assert_eq!(call(&result, "f", &[0]), "0");
    assert_eq!(call(&result, "f", &[7]), "2");
}

// Evidence written out, with the specification's examples (sections 7.3 and 8).

#[test]
fn matching_on_evidence_gives_each_arm_its_index_equations() {
    accepted(
        "prop SmallPrime(n: u8) {
            Two: @SmallPrime(2),
            Three: @SmallPrime(3),
            Five: @SmallPrime(5),
            Seven: @SmallPrime(7),
        }
        #[terminates] #[no_panic] #[no_io] fn small_prime_is_small(n: u8, h: @SmallPrime(n)) -> @(n <= 8) {
            match h {
                SmallPrime::Two => rewrite!(u8_eq_symm(n, 2, prove!(n == 2)), prove!(2u8 <= 8)),
                SmallPrime::Three => rewrite!(u8_eq_symm(n, 3, prove!(n == 3)), prove!(3u8 <= 8)),
                SmallPrime::Five => rewrite!(u8_eq_symm(n, 5, prove!(n == 5)), prove!(5u8 <= 8)),
                SmallPrime::Seven => rewrite!(u8_eq_symm(n, 7, prove!(n == 7)), prove!(7u8 <= 8)),
            }
        }
        #[terminates] #[no_panic] #[no_io] fn seven_is(h: @SmallPrime(7)) -> @(7u8 <= 8) { small_prime_is_small(7, h) }
        #[terminates] #[no_panic] #[no_io] fn five() -> @SmallPrime(5) { SmallPrime::Five }",
    );
    // The equations are what make the arms provable: each is a fact the
    // arm has, and the diagnostic names the step that uses it.
    let (codes, full) = rejected(
        "prop SmallPrime(n: u8) { Two: @SmallPrime(2), Seven: @SmallPrime(7) }
        #[terminates] #[no_panic] #[no_io] fn too_small(n: u8, h: @SmallPrime(n)) -> @(n <= 6) {
            match h { SmallPrime::Two => _, SmallPrime::Seven => _ }
        }",
    );
    // The `Two` arm is filled by arithmetic from its index equation `n ==
    // 2`; the `Seven` arm is false, and its equation is the counterexample.
    assert_eq!(codes, ["L0230"]);
    assert!(full.contains("cannot show `n <= 6`"), "{full}");
    assert!(full.contains("known here: `n == 7`"), "{full}");
    assert!(full.contains("it fails when n = 7"), "{full}");
}

#[test]
fn connectives_are_built_and_taken_apart_by_their_constructors() {
    accepted(
        "#[terminates] #[no_panic] #[no_io] fn swap(p: Prop, q: Prop, h: @(p || q)) -> @(q || p) {
            match h {
                Or::Left(hp) => Or::Right(hp),
                Or::Right(hq) => Or::Left(hq),
            }
        }
        #[terminates] #[no_panic] #[no_io] fn both(p: Prop, q: Prop, hp: @p, hq: @q) -> @(q && p) { And::Intro(hq, hp) }
        #[terminates] #[no_panic] #[no_io] fn first(p: Prop, q: Prop, h: @(p && q)) -> @p {
            match h { And::Intro(hp, _) => hp }
        }
        #[terminates] #[no_panic] #[no_io] fn anything(p: Prop, h: @(false)) -> @p { match h {} }
        fn unreachable(n: u8, h: @(false)) -> u8 { match h {} }",
    );
    // A hole takes a connective apart for no one: the search over them was
    // removed, and the diagnostic names the constructor or form instead.
    for (text, form) in [
        (
            "#[terminates] #[no_panic] #[no_io] fn swap(p: Prop, q: Prop, h: @(p && q)) -> @(q && p) { _ }",
            "`And::Intro(_, _)`",
        ),
        (
            "#[terminates] #[no_panic] #[no_io] fn weaken(p: Prop, q: Prop, hq: @q) -> @(p || q) { _ }",
            "`Or::Left(_)` or `Or::Right(_)`",
        ),
        (
            "#[terminates] #[no_panic] #[no_io] fn curry(p: Prop, q: Prop, hq: @q) -> @(p => q && q) { _ }",
            "evidence of `p => q` is a function of the logic",
        ),
        (
            "#[terminates] #[no_panic] #[no_io] fn modus(p: Prop, q: Prop, hp: @p, h: @(!p)) -> @(false) { _ }",
            "evidence of `false` is a refuted claim applied to its evidence",
        ),
        (
            "#[terminates] #[no_panic] #[no_io] fn ordered() -> @(forall (x: u8) { x <= 3 => x <= 3 }) { _ }",
            "evidence of `forall (x: T) { p }` is a function of the logic",
        ),
    ] {
        let (codes, full) = rejected(text);
        assert_eq!(codes, ["L0230"], "{text}");
        assert!(
            full.contains("the search over `&&`, `||`, `=>` and `forall` was removed"),
            "{text}: {full}"
        );
        assert!(full.contains(form), "{text}: {full}");
    }
}

#[test]
fn evidence_cannot_choose_a_value() {
    let (codes, full) = rejected(
        "fn pick(p: Prop, q: Prop, h: @(p || q)) -> u8 {
            match h { Or::Left(_) => 0, Or::Right(_) => 1 }
        }",
    );
    assert_eq!(codes, ["L0227"]);
    assert!(full.contains("only to produce other evidence"), "{full}");
}

#[test]
fn a_function_of_the_logic_is_evidence_of_its_general_claim_and_evidence_is_applied() {
    accepted(
        "#[terminates] #[no_panic] #[no_io] fn self_equal(x: u8) -> @(x == x) { _ }
        #[terminates] #[no_panic] #[no_io] fn all_self_equal() -> @(forall (x: u8) { x == x }) { self_equal }

        #[terminates] #[no_panic] #[no_io] fn at_most_nine_helper(limit: u8, h: @(limit <= 9), x: u8, hx: @(x <= limit)) -> @(x <= 9) {
            u8_le_trans(x, limit, 9, hx, h)
        }
        #[terminates] #[no_panic] #[no_io] fn at_most_nine(limit: u8, h: @(limit <= 9)) -> @(forall (x: u8) { x <= limit => x <= 9 }) {
            let general: @(forall (l: u8) { l <= 9 => forall (x: u8) { x <= l => x <= 9 } }) =
                at_most_nine_helper;
            general(limit)(h)
        }
        #[terminates] #[no_panic] #[no_io] fn use_it(h: @(forall (x: u8) { x <= 255 }), n: u8) -> @(n <= 255) { h(n) }",
    );
    let (codes, full) = rejected(
        "#[terminates] #[no_panic] #[no_io] fn f(h: @(1 == 1), n: u8) -> @(1 == 1) { h(n) }",
    );
    assert_eq!(codes, ["L0228"]);
    assert!(full.contains("takes no argument"), "{full}");
}

#[test]
fn rewrite_unfold_and_fold_are_the_explicit_forms() {
    accepted(
        "#[terminates] #[no_panic] #[no_io] fn nonzero(x: u8) -> Prop { prop!(x != 0) }
        #[terminates] #[no_panic] #[no_io] fn use_nonzero(n: u8, h: @nonzero(n)) -> @(n != 0) { unfold!(nonzero, h) }
        #[terminates] #[no_panic] #[no_io] fn make_nonzero(n: u8, h: @(n != 0)) -> @nonzero(n) { fold!(nonzero, h) }
        #[terminates] #[no_panic] #[no_io] fn moved(a: u8, b: u8, same: @(a == b), small: @(a <= 9)) -> @(b <= 9) {
            rewrite!(same, small)
        }",
    );
    let (codes, full) = rejected(
        "#[terminates] #[no_panic] #[no_io] fn nonzero(x: u8) -> Prop { prop!(x != 0) }
        #[terminates] #[no_panic] #[no_io] fn f(n: u8, h: @(n != 0)) -> @nonzero(n) { let folded = fold!(nonzero, h); folded }",
    );
    assert_eq!(codes, ["L0229"]);
    assert!(full.contains("needs to know the claim"), "{full}");
}

#[test]
fn a_constant_is_used_by_name() {
    let result = accepted(
        "const LIMIT: u8 = 3;
        const limit_is_small: Prop = prop!(LIMIT <= 9);
        #[terminates] #[no_panic] #[no_io] fn known() -> @limit_is_small { fold!(limit_is_small, prove!(LIMIT <= 9)) }
        fn clamp(n: u8) -> (out: u8, @(out <= LIMIT)) {
            if n <= LIMIT { (n, _) } else { (LIMIT, _) }
        }",
    );
    assert_eq!(call(&result, "clamp", &[2]), "(2, Proved)");
    assert_eq!(call(&result, "clamp", &[200]), "(3, Proved)");
}

#[test]
fn a_constructor_needs_to_know_what_it_proves() {
    let (codes, _) = rejected(
        "#[terminates] #[no_panic] #[no_io] fn f(p: Prop, q: Prop, hp: @p) -> () { let h = Or::Left(hp); () }",
    );
    assert_eq!(codes, ["L0226"]);
    accepted(
        "#[terminates] #[no_panic] #[no_io] fn f(p: Prop, q: Prop, hp: @p) -> () { let h: @(p || q) = Or::Left(hp); () }",
    );
}

// The built-in forms of S2: `prove!`, the respelled equality steps, and the
// names the respelling freed.

#[test]
fn prove_states_a_claim_where_it_stands_and_keeps_it_known() {
    // The `_` at the end has only the `prove!` statement to go on: `n < 10`
    // is what is known, and `n <= 9` is what the statement proved.
    let result = accepted(
        "fn bump(n: u8, small: @(n < 10)) -> (out: u8, @(out <= 10)) {
            let fits = u8_succ_le_of_lt(n, 10, small);
            prove!(n.wrapping_add(1) <= 10);
            let out = n.wrapping_add(1);
            let h = prove!(out <= 10);
            (out, h)
        }
        fn stated(n: u8, small: @(n < 10)) -> @(n.wrapping_add(1) <= 10) {
            let fits = u8_succ_le_of_lt(n, 10, small);
            prove!(n.wrapping_add(1) <= 10);
            _
        }
        fn valued(n: u8) -> (out: u8, @(out == n)) { (n, prove!(n == n)) }",
    );
    let module = result.session.erased();
    let bumped = Interpreter::new(module, FUEL)
        .call(
            result.function("bump").unwrap(),
            vec![Value::u8(9), Value::Proved],
        )
        .unwrap();
    assert_eq!(bumped.debug(module), "(10, Proved)");
    assert_eq!(call(&result, "valued", &[3]), "(3, Proved)");
    // Each `prove!` is one proof, as a `_` is; the `_` in `stated` found
    // the fact the statement left in scope.
    assert_eq!(result.holes.len(), 5);
    assert!(result.holes.iter().all(|hole| hole.solved));
    assert_eq!(result.holes[3].tier, "exact");
    // A statement erases to nothing, a value to the marker.
    let rust = print_module(result.session.erased());
    assert!(
        rust.contains(
            "    let fits = Proved;\n    let out = n.wrapping_add(1_u8);\n    let h = Proved;\n    (out, h)"
        ),
        "{rust}"
    );
    assert!(rust.contains("(n, Proved)"), "{rust}");
}

#[test]
fn a_failed_prove_is_reported_where_it_stands() {
    let (codes, full) = rejected("fn f(n: u8) -> u8 { prove!(n < 3); n }");
    assert_eq!(codes, ["L0230"]);
    assert!(full.contains("cannot show `n < 3`"), "{full}");
    assert!(full.contains("it fails when n = 3"), "{full}");
    let (codes, _) = rejected("fn f(n: u8) -> (out: u8, @(out < 3)) { (n, prove!(n < 3)) }");
    assert_eq!(codes, ["L0230"]);
    // Evidence of one claim where another is wanted is bridged as any
    // evidence is, and reported as such when it cannot be.
    let (codes, full) = rejected("fn f(n: u8) -> @(n <= 9) { prove!(n == n) }");
    assert_eq!(codes, ["L0230"]);
    assert!(full.contains("this is evidence of `n == n`"), "{full}");
}

#[test]
fn a_bare_rewrite_unfold_or_fold_gets_a_fix_that_elaborates() {
    let text = "#[terminates] #[no_panic] #[no_io] fn nonzero(x: u8) -> Prop { prop!(x != 0) }
        #[terminates] #[no_panic] #[no_io] fn use_nonzero(n: u8, h: @nonzero(n)) -> @(n != 0) { unfold(nonzero, h) }
        #[terminates] #[no_panic] #[no_io] fn make_nonzero(n: u8, h: @(n != 0)) -> @nonzero(n) { fold(nonzero, h) }
        #[terminates] #[no_panic] #[no_io] fn moved(a: u8, b: u8, same: @(a == b), small: @(a <= 9)) -> @(b <= 9) {
            rewrite(same, small)
        }";
    let result = elaborated(text);
    assert_eq!(result.diagnostics.len(), 3, "{:#?}", result.diagnostics);
    let mut fixed = text.to_owned();
    for diagnostic in result.diagnostics.iter().rev() {
        assert_eq!(diagnostic.code, "L0231");
        assert!(
            diagnostic.message.contains("written `"),
            "{}",
            diagnostic.message
        );
        let fix = &diagnostic.suggestions[0];
        assert_eq!(fix.replacement, "!");
        fixed.replace_range(fix.span.range(), &fix.replacement);
    }
    assert!(fixed.contains("unfold!(nonzero, h)"), "{fixed}");
    assert!(fixed.contains("fold!(nonzero, h)"), "{fixed}");
    assert!(fixed.contains("rewrite!(same, small)"), "{fixed}");
    accepted(&fixed);
    // The bare names are ordinary now.
    let result = accepted(
        "fn fold(n: u8) -> u8 { n }
        fn rewrite(n: u8, forall: u8) -> u8 { fold(n).wrapping_add(forall) }",
    );
    assert_eq!(call(&result, "rewrite", &[2, 3]), "5");
}

#[test]
fn the_forms_without_a_meaning_yet_say_which_task_brings_them() {
    for (text, form, task) in [
        (
            "fn f(n: u8) -> bool { matches!(n, 0) }",
            "matches",
            "LOC-71",
        ),
        ("fn f(n: u8) -> u8 { recurse!(n, n) }", "recurse", "LOC-53"),
        ("fn f(n: u8) -> u8 { vec!(1, 2) }", "vec", "Vec"),
    ] {
        let (codes, full) = rejected(text);
        assert_eq!(codes, ["L0290"], "{text}");
        assert!(
            full.contains(&format!("`{form}!` is not in Locus yet")),
            "{text}: {full}"
        );
        assert!(full.contains(task), "{text}: {full}");
    }
}

#[test]
fn quantifier_words_are_names_outside_a_formula_and_formulas_nest() {
    let result = accepted(
        "fn exists(n: u8) -> bool { n == 0 }
        fn f(forall: u8) -> u8 { let exists = forall; exists }
        const twice: Prop = prop!(forall (x: u8) { prop!(x == x) && prop!(prop!(0 <= x)) });
        #[terminates] #[no_panic] #[no_io] fn each(x: u8) -> @(x == x && 0 <= x) { And::Intro(prove!(x == x), u8_zero_le(x)) }
        #[terminates] #[no_panic] #[no_io] fn holds() -> @twice { fold!(twice, each) }
        #[terminates] #[no_panic] #[no_io] fn below(n: u8, x: u8, h: @(x <= n)) -> @(0 <= x) { u8_zero_le(x) }
        #[terminates] #[no_panic] #[no_io] fn general(n: u8) -> @(forall (x: u8) { x <= n => 0 <= x }) {
            let all: @(forall (m: u8) { forall (x: u8) { x <= m => 0 <= x } }) = below;
            all(n)
        }",
    );
    assert_eq!(call(&result, "exists", &[0]), "true");
    assert_eq!(call(&result, "f", &[7]), "7");
}

// --- Promises -----------------------------------------------------------------

/// The four promises, in the order a missing one is named in.
const PROMISES: [&str; 4] = ["terminates", "no_panic", "no_alloc", "no_io"];

/// A subset of the promises as a bit set, written as outer attributes.
fn outer(set: u8) -> String {
    PROMISES
        .iter()
        .enumerate()
        .filter(|(index, _)| set & (1 << index) != 0)
        .map(|(_, promise)| format!("#[{promise}] "))
        .collect()
}

/// A subset of the promises as `#![...]` lines at the top of a file.
fn inner(set: u8) -> String {
    PROMISES
        .iter()
        .enumerate()
        .filter(|(index, _)| set & (1 << index) != 0)
        .map(|(_, promise)| format!("#![{promise}]\n"))
        .collect()
}

/// The first promise `caller` makes that `callee` does not.
fn first_missing(caller: u8, callee: u8) -> Option<&'static str> {
    (0..4)
        .find(|index| caller & (1 << index) != 0 && callee & (1 << index) == 0)
        .map(|index| PROMISES[index])
}

/// Every pair of a caller's promises and a callee's: accepted exactly when
/// the callee makes every promise the caller does, and rejected naming the
/// first promise missing, at the call, with nothing else reported.
fn check_matrix(program: impl Fn(u8, u8) -> (String, u8), expected_accepted: usize) {
    let mut accepted_pairs = 0;
    for caller in 0..16u8 {
        for callee in 0..16u8 {
            let (text, effective_callee) = program(caller, callee);
            match first_missing(caller, effective_callee) {
                None => {
                    let result = accepted(&text);
                    assert_eq!(call(&result, "caller", &[3]), "3", "{text}");
                    accepted_pairs += 1;
                }
                Some(promise) => {
                    let result = elaborated(&text);
                    let codes: Vec<&str> = result.diagnostics.iter().map(|d| d.code).collect();
                    assert_eq!(codes, ["L0232"], "{text}");
                    let diagnostic = &result.diagnostics[0];
                    assert_eq!(
                        diagnostic.message,
                        format!("`caller` promises {promise} and calls `callee`, which does not"),
                        "{text}"
                    );
                    let span = diagnostic.labels[0].span;
                    assert_eq!(&text[span.range()], "callee(n)", "{text}");
                }
            }
        }
    }
    assert_eq!(accepted_pairs, expected_accepted);
}

#[test]
fn a_promise_is_kept_only_if_every_callee_makes_it() {
    check_matrix(
        |caller, callee| {
            let text = format!(
                "{}fn callee(n: u8) -> u8 {{ n }}\n{}fn caller(n: u8) -> u8 {{ callee(n) }}",
                outer(callee),
                outer(caller)
            );
            (text, callee)
        },
        // A pair is accepted when each promise is made by both, by the callee
        // only, or by neither: three ways for each of four promises.
        81,
    );
}

#[test]
fn a_file_default_is_made_by_every_function_and_an_attribute_adds_to_it() {
    // The file makes the first promise the caller makes, so both functions
    // make it; the rest of the caller's are written on the caller.
    check_matrix(
        |caller, callee| {
            let file = caller & caller.wrapping_neg();
            let text = format!(
                "{}{}fn callee(n: u8) -> u8 {{ n }}\n{}fn caller(n: u8) -> u8 {{ callee(n) }}",
                inner(file),
                outer(callee & !file),
                outer(caller & !file)
            );
            (text, callee | file)
        },
        // The file adds the caller's first promise to the callee, so one promise
        // of every nonempty caller set is never missing: for a caller with k
        // promises, 2^(5 - k) callee sets; 16 for the empty one.
        146,
    );
    // The whole set at the top of the file, and nothing on the functions.
    let result = accepted(
        "#![terminates]
        #![no_panic]
        #![no_alloc]
        #![no_io]
        fn callee(n: u8) -> u8 { n }
        fn caller(n: u8) -> u8 { callee(n) }
        fn claim(n: u8) -> @(callee(n) == n) { fold!(callee, prove!(n == n)) }",
    );
    assert_eq!(call(&result, "caller", &[3]), "3");
}

#[test]
fn a_function_appears_in_a_proposition_exactly_when_it_promises_the_three() {
    let places = [
        "fn g(n: u8) -> @(f(n) == n) { _ }",
        "fn g(n: u8) -> Prop { prop!(f(n) == n) }",
        "fn g(n: u8) -> @(n == n) { prove!(f(n) == n); _ }",
        "fn g(n: u8, h: @(n == n)) -> @(n == n) { unfold!(f, h) }",
        "fn g(n: u8, h: @(n == n)) -> @(n == n) { fold!(f, h) }",
        "prop P(n: u8) { Is(m: u8): @P(f(m)) }",
        "const C: u8 = f(1);",
    ];
    for (attributes, missing) in [
        ("", "terminates"),
        ("#[terminates]", "no_panic"),
        ("#[terminates] #[no_panic]", "no_io"),
        ("#[no_panic] #[no_io]", "terminates"),
        ("#[terminates] #[no_alloc] #[no_io]", "no_panic"),
    ] {
        for place in places {
            let text = format!("{attributes} fn f(n: u8) -> u8 {{ n }}\n{place}");
            let (codes, full) = rejected(&text);
            assert_eq!(codes, ["L0209"], "{text}: {full}");
            let where_ = if place.starts_with("const") {
                "the value of a constant"
            } else {
                "a proposition"
            };
            assert!(
                full.starts_with(&format!(
                    "`f` cannot appear in {where_}: it does not promise {missing}"
                )),
                "{text}: {full}"
            );
        }
    }
    // With the three it is admitted, its defining equation is known, and
    // it still runs; `no_alloc` is not needed and may be added.
    for attributes in [
        "#[terminates] #[no_panic] #[no_io]",
        "#[no_io] #[no_alloc] #[no_panic] #[terminates]",
        "#[no_alloc] #[terminates] #[no_panic] #[no_io]",
    ] {
        let text = format!(
            "{attributes} fn f(n: u8) -> u8 {{ n.wrapping_add(1) }}
            fn back(n: u8, h: @(f(n) == 4)) -> @(n.wrapping_add(1) == 4) {{ unfold!(f, h) }}
            fn forth(n: u8, h: @(n.wrapping_add(1) == 4)) -> @(f(n) == 4) {{ fold!(f, h) }}
            fn known(n: u8) -> (out: u8, @(out == f(n))) {{ let out = f(n); (out, _) }}
            fn stated() -> @(f(3) == 4) {{ _ }}
            prop Next(n: u8) {{ Is(m: u8): @Next(f(m)) }}
            const FIVE: Prop = prop!(f(4) == 5);"
        );
        let result = accepted(&text);
        assert_eq!(call(&result, "known", &[3]), "(4, Proved)");
        assert_eq!(call(&result, "f", &[9]), "10");
    }
}

#[test]
fn a_loop_of_any_form_is_refused_under_terminates_at_its_keyword() {
    let bodies = [
        ("loop { break n }", "loop"),
        ("let mut a: u8 = 0; for i in 0..n { a = i; } a", "for"),
        // The removed state-passing forms: the promise is what is reported.
        ("loop () -> u8 { break n }", "loop"),
        ("while n < 3 { } n", "while"),
    ];
    for (body, keyword) in bodies {
        for head in [
            "#[terminates] fn",
            "#[terminates] #[no_panic] fn",
            "#[terminates] #[no_panic] #[no_io] fn",
        ] {
            let text = format!("{head} f(n: u8) -> u8 {{ {body} }}");
            let result = elaborated(&text);
            let codes: Vec<&str> = result.diagnostics.iter().map(|d| d.code).collect();
            assert_eq!(codes, ["L0215"], "{text}");
            let diagnostic = &result.diagnostics[0];
            assert_eq!(
                diagnostic.message,
                format!("`{keyword}` cannot appear in `f`, which promises terminates"),
                "{text}"
            );
            assert_eq!(&text[diagnostic.labels[0].span.range()], keyword, "{text}");
        }
        let text = format!("#![terminates]\nfn f(n: u8) -> u8 {{ {body} }}");
        let (codes, _) = rejected(&text);
        assert_eq!(codes, ["L0215"], "{text}");
    }
    // Without the promise Rust's forms are accepted and the state-passing
    // ones were removed.
    accepted("fn f(n: u8) -> u8 { loop { break n } }");
    accepted("fn f(n: u8) -> u8 { while n < 3 { } n }");
    let (codes, full) = rejected("fn f(n: u8) -> u8 { loop () -> u8 { break n } }");
    assert_eq!(codes, ["L0234"]);
    assert!(full.contains("was removed"), "{full}");
    // Nothing in a proposition runs.
    let (codes, full) = rejected("fn f(n: u8) -> Prop { prop!(loop { break n } == n) }");
    assert_eq!(codes, ["L0215"]);
    assert!(
        full.starts_with("`loop` cannot appear in a proposition"),
        "{full}"
    );
}

#[test]
fn a_promise_goes_on_a_function_and_decreases_waits_for_recursion() {
    for (text, what) in [
        ("#[no_panic] struct S { n: u8 }", "`S` is a struct"),
        ("#[terminates] enum E { A }", "`E` is an enum"),
        ("#[no_io] prop P(n: u8) { Is }", "`P` is a proposition"),
        ("#[no_alloc] const C: u8 = 1;", "`C` is a constant"),
    ] {
        let (codes, full) = rejected(text);
        assert_eq!(codes, ["L0233"], "{text}");
        assert!(full.contains(what), "{text}: {full}");
    }
    let (codes, full) = rejected("#[terminates(decreases = n)] fn f(n: u8) -> u8 { n }");
    assert_eq!(codes, ["L0290"]);
    assert!(full.contains("Recursion"), "{full}");
    // It counts as the bare promise meanwhile.
    let (codes, full) =
        rejected("fn g(n: u8) -> u8 { n } #[terminates(decreases = n)] fn f(n: u8) -> u8 { g(n) }");
    assert_eq!(codes, ["L0290", "L0232"]);
    assert!(full.contains("Recursion"), "{full}");
}

/// The elaborator's check is the friendly one; the checker's word counts.
/// A typed tree that claims a promise and calls a function without it is
/// refused by the checker with the promise and the callee named.
#[test]
fn the_checker_refuses_a_promise_the_elaborator_did_not_check() {
    use locus::exec::{ExecError, Promise, Promises};
    use locus::kernel::{Definitions, HypId, Type, VarId};
    use locus::typed::{Binder, Block, Expr, FnItem, FnRef, LowerError, Pattern, Session, Stmt};

    let (definitions, _) = Definitions::with_prelude();
    let mut session = Session::new(definitions);
    let callee = FnItem {
        passing: Vec::new(),
        exits: Vec::new(),
        name: "callee".into(),
        math: false,
        params: vec![],
        result: Type::U8,
        body: Block {
            stmts: vec![],
            tail: Some(Box::new(Expr::u8(1))),
        },
    };
    let FnRef::Exec(callee_id) = session
        .declare_fn_promising(&callee, Promises::default())
        .expect("the callee is well formed")
    else {
        panic!("an ordinary function")
    };
    let caller = |name: &str| {
        let out = Binder::new("out", Type::U8);
        FnItem {
            passing: Vec::new(),
            exits: Vec::new(),
            name: name.into(),
            math: false,
            params: vec![],
            result: Type::U8,
            body: Block {
                stmts: vec![Stmt::Let {
                    pattern: Pattern::Bind {
                        binder: out.clone(),
                        equation: HypId::fresh(),
                        mutable: false,
                    },
                    value: Expr::CallFn {
                        lends: Vec::new(),
                        id: callee_id,
                        name: "callee".into(),
                        arguments: vec![],
                        result: VarId::fresh(),
                        ty: Type::U8,
                    },
                }],
                tail: Some(Box::new(Expr::var(&out))),
            },
        }
    };
    // Without a claim the call is fine.
    session
        .declare_fn_promising(&caller("plain"), Promises::default())
        .expect("a function without promises calls anything");
    for (promise, claimed) in [
        (
            Promise::Terminates,
            Promises {
                terminates: true,
                ..Promises::default()
            },
        ),
        (
            Promise::NoPanic,
            Promises {
                no_panic: true,
                ..Promises::default()
            },
        ),
        (
            Promise::NoAlloc,
            Promises {
                no_alloc: true,
                ..Promises::default()
            },
        ),
        (
            Promise::NoIo,
            Promises {
                no_io: true,
                ..Promises::default()
            },
        ),
    ] {
        let refused = session.declare_fn_promising(&caller("claims"), claimed);
        assert_eq!(
            refused.err(),
            Some(LowerError::Exec(ExecError::CalleeBreaksPromise {
                promise,
                callee: callee_id,
            }))
        );
    }
}

#[test]
fn a_type_and_a_value_of_one_name_coexist_as_in_rust() {
    // The smallest case the one-namespace elaborator rejected as declared
    // twice: an enum and a function of its name. Where a type is read the
    // name is the enum, where a value is read it is the function, and the
    // Rust has both (rustc accepts it, with a lint about the case).
    let result = accepted(
        "enum Mode { Off, On }
        fn Mode(on: bool) -> Mode { if on { Mode::On } else { Mode::Off } }
        fn is_on(mode: Mode) -> bool { match mode { Mode::On => true, Mode::Off => false } }
        fn check(on: bool) -> bool { is_on(Mode(on)) }
        struct Point { x: u8 }
        const Point: Point = Point { x: 3 };
        fn x() -> u8 { Point.x }",
    );
    assert_eq!(call(&result, "x", &[]), "3");
    let rust = print_module(result.session.erased());
    assert!(rust.contains("pub enum Mode {"), "{rust}");
    assert!(rust.contains("pub fn Mode(on: bool) -> Mode {"), "{rust}");
    assert!(rust.contains("is_on(Mode(on))"), "{rust}");
    assert!(
        rust.contains("pub const Point: Point = Point { x: 3_u8 };"),
        "{rust}"
    );
    assert!(rust.contains("Point.x"), "{rust}");
    // Within a namespace a name is still declared once.
    for text in [
        "enum Mode { Off } struct Mode { x: u8 }",
        "fn f() -> u8 { 1 } const f: u8 = 1;",
        "prop P { Yes } enum P { No }",
    ] {
        let (codes, _) = rejected(text);
        assert!(codes.contains(&"L0202"), "{text}: {codes:?}");
    }
    // A call `Name(..)` is the function when there is one, and the
    // proposition otherwise.
    accepted(
        "prop Small(n: u8) { Below(bound: @(n < 10)) }
        #[terminates] #[no_panic] #[no_io]
        fn small(n: u8, bound: @(n < 10)) -> @Small(n) { Small::Below(bound) }",
    );
}

#[test]
fn a_variant_with_named_fields_is_a_tuple_variant_with_names_in_the_logic() {
    // Positions in the kernel, names in the source: the claim about `width`
    // holds by computing, and evidence fields are fields.
    let result = accepted(
        "enum Shape { Box { width: u8, height: u8 }, Bounded { limit: u8, value: u8, fits: @(value <= limit) } }
        #[terminates] #[no_panic] #[no_io]
        fn width(shape: Shape) -> u8 {
            match shape { Shape::Box { width, .. } => width, Shape::Bounded { value, .. } => value }
        }
        fn square(side: u8) -> Shape { Shape::Box { height: side, width: side } }
        fn width_of_box() -> @(width(Shape::Box { width: 3, height: 4 }) == 3) { _ }
        fn bounded(limit: u8, value: u8, fits: @(value <= limit)) -> Shape { Shape::Bounded { limit, value, fits } }",
    );
    assert!(result.holes.iter().all(|hole| hole.solved));
    let rust = print_module(result.session.erased());
    assert!(rust.contains("Box { width: u8, height: u8 }"), "{rust}");
    assert!(
        rust.contains("Bounded { limit: u8, value: u8, fits: Proved }"),
        "{rust}"
    );
    assert!(rust.contains("Shape::Box { width, .. } =>"), "{rust}");
    assert!(
        rust.contains("Shape::Box { width: side, height: side }"),
        "{rust}"
    );
    assert!(
        rust.contains("Shape::Bounded { limit, value, fits }"),
        "{rust}"
    );
}

#[test]
fn derives_are_printed_as_written_and_nothing_else_is_added() {
    // O1: the closed list, in the order given; a type without a derive
    // gets no attribute, since `Copy` would let a moved value be reused.
    let result = accepted(
        "#[derive(Debug, Clone, Copy)] struct Point { x: u8, y: u8 }
        #[derive(Clone, PartialEq, Eq, Debug)] enum Mode { Off, On(u8) }
        struct Token { id: u8 }
        fn origin() -> Point { Point { x: 0, y: 0 } }",
    );
    let rust = print_module(result.session.erased());
    assert!(
        rust.contains("#[derive(Debug, Clone, Copy)]\npub struct Point {"),
        "{rust}"
    );
    assert!(
        rust.contains("#[derive(Clone, PartialEq, Eq, Debug)]\npub enum Mode {"),
        "{rust}"
    );
    assert!(rust.contains("}\n\npub struct Token {"), "{rust}");
}

#[test]
fn a_copy_value_is_reused_and_a_moved_one_is_not() {
    // The fifth case of the mutation design: move and reinitialise.
    let result = accepted(
        "struct Token { id: u8 }
        fn consume(t: Token) -> u8 { t.id }
        fn twice(a: u8, b: u8) -> u8 {
            let mut t = Token { id: a };
            let first = consume(t);
            t = Token { id: b };
            first.wrapping_add(consume(t))
        }",
    );
    assert_eq!(call(&result, "twice", &[3, 4]), "7");
    let (codes, message) = rejected(
        "struct Token { id: u8 }
        fn consume(t: Token) -> u8 { t.id }
        fn twice(a: u8) -> u8 {
            let t = Token { id: a };
            let first = consume(t);
            first.wrapping_add(consume(t))
        }",
    );
    assert_eq!(codes, ["L0240"]);
    assert!(message.starts_with("use of moved value: `t`"), "{message}");
    // With `Copy`, the same program is accepted.
    let result = accepted(
        "#[derive(Clone, Copy)] struct Token { id: u8 }
        fn consume(t: Token) -> u8 { t.id }
        fn twice(a: u8) -> u8 {
            let t = Token { id: a };
            let first = consume(t);
            first.wrapping_add(consume(t))
        }",
    );
    assert_eq!(call(&result, "twice", &[3]), "6");
}

#[test]
fn a_lemma_reads_its_argument_and_a_proposition_cannot_mention_a_moved_one() {
    // The argument of a call erasure removes is a reading: the value stays
    // whole for the code after it.
    accepted(
        "struct Token { id: u8 }
        #[terminates] #[no_panic] #[no_io]
        fn small(t: Token) -> Prop { prop!(t.id <= 10) }
        fn consume(t: Token) -> u8 { t.id }
        fn read_then_use(n: u8) -> u8 {
            let t = Token { id: n };
            let claim = small(t);
            let again = prop!(small(t) && t.id == n);
            consume(t)
        }",
    );
    let (codes, message) = rejected(
        "struct Token { id: u8 }
        fn consume(t: Token) -> u8 { t.id }
        fn afterwards(n: u8) -> u8 {
            let t = Token { id: n };
            let out = consume(t);
            prove!(t.id == n);
            out
        }",
    );
    assert_eq!(codes, ["L0241"]);
    assert!(message.contains("`t` was moved at line 5"), "{message}");
}

// --- The forms that panic (E10) --------------------------------------------------------

/// A call in the erased-tree interpreter, whatever it ends in.
fn outcome(result: &Elaborated, name: &str, arguments: Vec<Value>) -> Outcome {
    let module = result.session.erased();
    let function = result.function(name).expect("the function exists");
    Interpreter::new(module, FUEL)
        .call(function, arguments)
        .expect("the program runs")
}

fn panicked(message: &str) -> Outcome {
    Outcome::Panic(message.into())
}

fn returned(value: u8) -> Outcome {
    Outcome::Value(Value::u8(value))
}

#[test]
fn the_forms_that_panic_yield_no_value_and_take_the_type_expected_of_them() {
    // `panic!`, `todo!`, and `unreachable!` are the never type: a branch
    // that panics takes the other's type, and a `let` its annotation.
    let result = accepted(
        "fn loud(c: bool) -> u8 { if c { panic!(\"x\") } else { 5 } }
        fn quiet(c: bool) -> u8 { if c { panic!() } else { 5 } }
        fn half(c: bool) -> u8 { if c { todo!() } else { todo!(\"later\") } }
        fn dead(c: bool) -> (out: u8, @(out <= 5)) { if c { (5, _) } else { unreachable!(\"c holds\") } }
        fn bound() -> u8 { let x: u8 = todo!(); x.wrapping_add(1) }
        fn evidence(n: u8) -> @(n <= 9) { unreachable!() }
        fn checked(x: u8) -> u8 { assert!(x <= 3); x }
        fn named(x: u8) -> u8 { assert!(x != 0, \"x must not be zero\"); debug_assert!(x <= 200); x }",
    );
    // Rust's messages, exactly.
    let (yes, no) = (Value::Bool(true), Value::Bool(false));
    assert_eq!(outcome(&result, "loud", vec![yes.clone()]), panicked("x"));
    assert_eq!(outcome(&result, "loud", vec![no.clone()]), returned(5));
    assert_eq!(
        outcome(&result, "quiet", vec![yes.clone()]),
        panicked("explicit panic")
    );
    assert_eq!(
        outcome(&result, "half", vec![yes.clone()]),
        panicked("not yet implemented")
    );
    assert_eq!(
        outcome(&result, "half", vec![no.clone()]),
        panicked("not yet implemented: later")
    );
    assert_eq!(
        outcome(&result, "dead", vec![no]),
        panicked("internal error: entered unreachable code: c holds")
    );
    assert_eq!(
        outcome(&result, "bound", vec![]),
        panicked("not yet implemented")
    );
    assert_eq!(
        outcome(&result, "evidence", vec![Value::u8(1)]),
        panicked("internal error: entered unreachable code")
    );
    assert_eq!(
        outcome(&result, "checked", vec![Value::u8(4)]),
        panicked("assertion failed: x <= 3")
    );
    assert_eq!(outcome(&result, "checked", vec![Value::u8(3)]), returned(3));
    assert_eq!(
        outcome(&result, "named", vec![Value::u8(0)]),
        panicked("x must not be zero")
    );
    assert_eq!(
        outcome(&result, "named", vec![Value::u8(201)]),
        panicked("assertion failed: x <= 200")
    );
    // Printed as written, with the message Rust's form ends with.
    let rust = print_module(result.session.erased());
    for line in [
        "panic!(\"{}\", \"x\")",
        "panic!()",
        "todo!()",
        "todo!(\"{}\", \"later\")",
        "unreachable!(\"{}\", \"c holds\")",
        "let x: u8 = todo!();",
        "unreachable!()",
        "assert!(x <= 3_u8, \"{}\", \"assertion failed: x <= 3\");",
        "assert!(x != 0_u8, \"{}\", \"x must not be zero\");",
        "debug_assert!(x <= 200_u8, \"{}\", \"assertion failed: x <= 200\");",
    ] {
        assert!(rust.contains(line), "{line}\n{rust}");
    }
}

#[test]
fn after_an_assertion_its_condition_is_a_fact_and_a_debug_assertion_teaches_nothing() {
    // The check passed, so `x <= 3` holds: `prove!` finds it without any
    // promise, by reflecting the outcome the check's statement is evidence
    // of.
    let result = accepted(
        "fn f(x: u8) -> (out: u8, @(out <= 3)) { assert!(x <= 3); prove!(x <= 3); (x, _) }
        fn g(x: u8, y: u8) -> @(x != y) { assert!(x != y); _ }
        fn h(b: bool) -> @(b == true) { assert!(b); _ }",
    );
    assert!(result.holes.iter().all(|hole| hole.solved));
    assert_eq!(result.holes.len(), 4);
    // Without the check, nothing shows it; and a `debug_assert!` is not
    // checked in every build, so it teaches nothing either.
    let (codes, full) = rejected("fn f(x: u8) -> u8 { prove!(x <= 3); x }");
    assert_eq!(codes, ["L0230"], "{full}");
    let (codes, full) = rejected("fn f(x: u8) -> u8 { debug_assert!(x <= 3); prove!(x <= 3); x }");
    assert_eq!(codes, ["L0230"], "{full}");
}

#[test]
fn under_no_panic_each_form_is_refused_or_needs_its_evidence() {
    let cases = [
        (
            "#[no_panic] fn f(n: u8) -> u8 { if n == 0 { panic!(\"zero\") } else { n } }",
            "`panic!` cannot appear in `f`, which promises no_panic",
        ),
        (
            "#[no_panic] fn f(n: u8) -> u8 { todo!() }",
            "`todo!` panics, and `f` promises no_panic",
        ),
        (
            "#[no_panic] fn f(x: u8) -> u8 { assert!(x <= 3); x }",
            "`assert!` needs evidence of its condition in `f`, which promises no_panic: cannot show `x <= 3`",
        ),
        (
            "#[no_panic] fn f(x: u8, h: @(x <= 3)) -> u8 { debug_assert!(x <= 2); x }",
            "`debug_assert!` needs evidence of its condition in `f`, which promises no_panic: cannot show `x <= 2`",
        ),
        (
            "#[no_panic] fn f(n: u8) -> u8 { if n == 0 { unreachable!() } else { n } }",
            "`unreachable!()` needs evidence that this point is unreachable in `f`, which promises no_panic: cannot show `false`",
        ),
    ];
    for (text, message) in cases {
        let (codes, full) = rejected(text);
        assert_eq!(codes, ["L0239"], "{text}: {full}");
        assert!(full.contains(message), "{text}: {full}");
    }
    // The unsolved hole's notes come with the form's message.
    let (_, full) = rejected("#[no_panic] fn f(x: u8) -> u8 { assert!(x <= 3); x }");
    assert!(full.contains("it fails when x = 4"), "{full}");
}

#[test]
fn under_no_panic_the_evidence_is_found_as_a_hole_is_filled_and_the_checker_verifies_it() {
    // `assert!(c)` from a hypothesis, by arithmetic, and from the branch
    // taken; `unreachable!()` from contradictory facts, and from `False`.
    let result = accepted(
        "#[no_panic] fn exact(x: u8, small: @(x <= 3)) -> u8 { assert!(x <= 3); x }
        #[no_panic] fn apart(x: u8, small: @(x <= 3)) -> u8 { assert!(x != 4); assert!(x < 4); x }
        #[no_panic] fn same(x: u8, three: @(x == 3)) -> u8 { assert!(x == 3); x }
        #[no_panic] fn branch(b: bool) -> u8 { if b { assert!(b); 1 } else { 0 } }
        #[no_panic] fn dead(x: u8, small: @(x <= 3)) -> u8 { if x <= 3 { x } else { unreachable!() } }
        #[no_panic] fn absurd(no: @(false)) -> u8 { unreachable!() }
        #[no_panic] fn taught(x: u8, small: @(x <= 3)) -> (out: u8, @(out <= 3)) { assert!(x <= 3); (x, _) }",
    );
    assert!(
        result.holes.iter().all(|hole| hole.solved),
        "{:#?}",
        result.holes
    );
    let tiers: Vec<&str> = result.holes.iter().map(|hole| hole.tier).collect();
    assert_eq!(
        tiers,
        [
            "exact",
            "arithmetic",
            "arithmetic",
            "exact",
            "exact",
            "arithmetic",
            "exact",
            "exact",
            "exact"
        ]
    );
    // The checks still run.
    assert_eq!(
        outcome(&result, "exact", vec![Value::u8(3), Value::Proved]),
        returned(3)
    );
    assert_eq!(
        outcome(&result, "branch", vec![Value::Bool(false)]),
        returned(0)
    );
}

#[test]
fn without_the_promise_the_evidence_is_tried_and_kept_when_found() {
    // Found: the proof is attached and counted, and the kernel checks it.
    // Not found: the form panics as Rust's does, and no hole is reported.
    let found = accepted(
        "fn dead(x: u8, small: @(x <= 3)) -> u8 { if x <= 3 { x } else { unreachable!() } }",
    );
    assert_eq!(found.holes.len(), 1);
    assert_eq!(found.holes[0].tier, "arithmetic");
    let tried = accepted("fn dead(x: u8) -> u8 { if x <= 3 { x } else { unreachable!() } }");
    assert!(tried.holes.is_empty(), "{:#?}", tried.holes);
    assert_eq!(
        outcome(&tried, "dead", vec![Value::u8(4)]),
        panicked("internal error: entered unreachable code")
    );
}

#[test]
fn a_message_is_a_string_literal_and_the_forms_stand_nowhere_that_nothing_runs() {
    let cases = [
        (
            "fn f() -> u8 { assert!(); 1 }",
            "L0244",
            "`assert!` takes a condition",
        ),
        (
            "fn f(n: u8) -> u8 { panic!(n) }",
            "L0244",
            "the message of `panic!` is a string literal",
        ),
        (
            "fn f(n: u8) -> u8 { assert!(n <= 3, n); n }",
            "L0244",
            "the message of `assert!` is a string literal",
        ),
        (
            "fn f(n: u8) -> u8 { panic!(\"n is {}\", n) }",
            "L0290",
            "format arguments are not in Locus yet",
        ),
        (
            "fn f(n: u8) -> u8 { todo!(\"{}\", n) }",
            "L0290",
            "format arguments are not in Locus yet",
        ),
        (
            "fn f(n: u8) -> Prop { prop!(n == todo!()) }",
            "L0239",
            "`todo!` cannot appear in a proposition: nothing there runs",
        ),
        (
            "fn f(n: u8) -> @(n <= todo!()) { _ }",
            "L0239",
            "cannot appear in a proposition",
        ),
    ];
    for (text, code, fragment) in cases {
        let (codes, full) = rejected(text);
        assert_eq!(codes, [code], "{text}: {full}");
        assert!(full.contains(fragment), "{text}: {full}");
    }
    // A function of the logic with a check in its body is elaborated again
    // as an ordinary one with its promises (LOC-193), and is then known by
    // its contract only.
    let result = accepted(
        "#[terminates] #[no_panic] #[no_io] fn f(x: u8, small: @(x <= 3)) -> u8 { assert!(x <= 3); x }",
    );
    assert!(result.holes.iter().all(|hole| hole.solved));
    let (codes, full) = rejected(
        "#[terminates] #[no_panic] #[no_io] fn f(x: u8, small: @(x <= 3)) -> u8 { assert!(x <= 3); x }
        fn g(small: @(2u8 <= 3)) -> @(f(2, small) == 2) { _ }",
    );
    assert_eq!(codes, ["L0209"], "{full}");
    assert!(full.contains("`assert!`"), "{full}");
}

// --- Logic-only types and the one erasure rule (E8) ------------------------------

#[test]
fn a_pure_call_in_a_logic_only_context_is_absent_from_the_generated_rust() {
    // A function of the logic in the value of a `Ghost<T>` let, and an
    // ordinary function with the three promises (the interim rule of
    // LOC-193) whose call returns only evidence: neither call is in the
    // Rust, only the definitions and the runtime call of `double`.
    let result = accepted(
        "#[terminates] #[no_panic] #[no_io]
        fn double(n: u8) -> u8 { n.wrapping_add(n) }
        #[terminates] #[no_panic] #[no_io]
        fn bounded_by(n: u8, h: @(n <= 200)) -> @(n as Int + 50 <= 255) { let s = n + 50; _ }
        fn noted(n: u8, h: @(n <= 200)) -> (out: u8, @(out == double(n))) {
            let noted: Ghost<u8> = double(n);
            let room = bounded_by(n, h);
            let out = double(n);
            (out, prove!(out == noted))
        }",
    );
    let source = print_module(result.session.erased());
    assert_eq!(source.matches("double(").count(), 2, "{source}");
    assert_eq!(source.matches("bounded_by(").count(), 1, "{source}");
    assert!(source.contains("let room = Proved;"), "{source}");
    assert!(!source.contains("let noted"), "{source}");
    assert!(!source.contains("Ghost<"), "{source}");
}

#[test]
fn a_call_in_a_logic_only_context_must_be_one_a_proposition_admits() {
    // The three promises, each missing in turn, in each logic-only context:
    // the error names the context and the promise.
    for (attributes, missing) in [
        ("", "terminates"),
        ("#[terminates]", "no_panic"),
        ("#[terminates] #[no_panic]", "no_io"),
    ] {
        for (statement, place) in [
            ("let g = snapshot!(f(n));", "the argument of `snapshot!`"),
            (
                "let g: Ghost<u8> = f(n);",
                "the value of a `let` with no runtime form",
            ),
            (
                "let g: Int = f(n) as Int;",
                "the value of a `let` with no runtime form",
            ),
            ("let g: Prop = prop!(f(n) == n);", "a proposition"),
            ("let g = takes(n, f(n));", "a `Ghost<T>` argument"),
        ] {
            let text = format!(
                "{attributes} fn f(n: u8) -> u8 {{ n }}
                fn takes(n: u8, cap: Ghost<u8>) -> u8 {{ n }}
                fn g(n: u8) -> u8 {{ {statement} n }}"
            );
            let (codes, full) = rejected(&text);
            assert_eq!(codes, ["L0209"], "{text}: {full}");
            assert!(
                full.starts_with(&format!(
                    "`f` cannot appear in {place}: it does not promise {missing}"
                )),
                "{text}: {full}"
            );
        }
    }
}

#[test]
fn a_runtime_call_returning_only_evidence_stays_and_its_panic_is_seen() {
    // An erased result is not an erasable computation: `checked` returns
    // only evidence and may panic, so the call stays in the Rust and the
    // interpreter sees the panic through it.
    let result = accepted(
        "fn checked(x: u8) -> @(x <= 255) { let _ = 255u8 + x; _ }
        fn use_checked(x: u8) -> u8 { let h = checked(x); x }",
    );
    let source = print_module(result.session.erased());
    assert!(source.contains("let h = checked(x);"), "{source}");
    assert_eq!(call(&result, "use_checked", &[0]), "0");
    let module = result.session.erased();
    let function = result.function("use_checked").unwrap();
    let outcome = Interpreter::new(module, FUEL)
        .call(function, vec![Value::u8(200)])
        .unwrap();
    assert!(
        matches!(outcome, locus::erased::Outcome::Panic(_)),
        "{outcome:?}"
    );
}

// A `&mut` parameter is assigned as a value in its body, `x = 0` (O3); the
// spelling `*x = 0` waits for the `*` of `&mut self` (O4).
#[test]
fn a_call_with_a_logic_only_result_and_a_mut_parameter_is_kept() {
    let result = accepted(
        "#[terminates] #[no_panic] #[no_io]
        fn clear(x: &mut u8) -> @(x == 0) { x = 0; _ }
        fn use_clear(n: u8) -> u8 { let mut x = n; let h = clear(&mut x); x }",
    );
    let source = print_module(result.session.erased());
    assert!(source.contains("let h = clear(&mut x);"), "{source}");
    assert_eq!(call(&result, "use_clear", &[7]), "0");
}

#[test]
fn a_ghost_value_is_named_only_where_nothing_runs() {
    // In a proposition a `Ghost<T>` reads as its `T` value, and `snapshot!`
    // of a value that would move is a reading of it.
    accepted(
        "struct Token { id: u8 }
        fn consume(t: Token) -> u8 { t.id }
        fn keep(n: u8) -> (out: u8, @(out == n)) {
            let t = Token { id: n };
            let before: Ghost<u8> = t.id;
            let was = snapshot!(t);
            let consumed = consume(t);
            prove!(before == n);
            prove!(was.id == n);
            (n, _)
        }",
    );
    for (text, message) in [
        (
            "fn f(n: u8) -> u8 { let g = snapshot!(n); g }",
            "`g` is a `Ghost<u8>`, which has no runtime form",
        ),
        (
            "fn f(n: u8) -> u8 { snapshot!(n) }",
            "`snapshot!` builds a `Ghost<T>`, which has no runtime form",
        ),
        (
            "struct H { value: Ghost<u8> } fn f(h: H) -> u8 { h.value }",
            "`h.value` is a `Ghost<u8>`, which has no runtime form",
        ),
        (
            "fn f(n: u8, cap: Ghost<u8>) -> u8 { let y: u8 = cap; y }",
            "`cap` is a `Ghost<u8>`, which has no runtime form",
        ),
    ] {
        let (codes, full) = rejected(text);
        assert_eq!(codes, ["L0201"], "{text}: {full}");
        assert!(full.starts_with(message), "{text}: {full}");
    }
}

// --- return and the never type (M5) ---------------------------------------------------

fn unit() -> Outcome {
    Outcome::Value(Value::Tuple(Vec::new()))
}

#[test]
fn a_return_ends_the_function_from_any_depth() {
    // Three ways out: a guard in statement position, an arm of a branch,
    // and the tail; then from a loop inside a branch, in each loop form.
    let result = accepted(
        "fn guarded(n: u8) -> u8 { if n == 0 { return 1; } else { } n }
        fn as_arm(n: u8) -> u8 { if n == 0 { return 1 } else { n } }
        fn as_tail(n: u8) -> u8 { return n.wrapping_add(1) }
        fn nothing(n: u8) -> () { if n == 0 { return; } else { } }
        fn find(limit: u8) -> u8 {
            let mut i: u8 = 0;
            loop { if i == limit { return i; } else { } i = i.wrapping_add(1); }
        }
        fn first_over(limit: u8) -> u8 {
            for i in 0..limit { if i.wrapping_mul(2) > limit { return i; } else { } }
            limit
        }
        fn counted(n: u8) -> u8 {
            let mut seen: u8 = 0;
            while seen < n { if seen == 7 { return 100; } else { } seen = seen.wrapping_add(1); }
            seen
        }",
    );
    for (name, argument, expected) in [
        ("guarded", 0, 1),
        ("guarded", 5, 5),
        ("as_arm", 0, 1),
        ("as_arm", 5, 5),
        ("as_tail", 5, 6),
        ("find", 4, 4),
        ("first_over", 5, 3),
        ("first_over", 0, 0),
        ("counted", 3, 3),
        ("counted", 9, 100),
    ] {
        assert_eq!(
            outcome(&result, name, vec![Value::u8(argument)]),
            returned(expected),
            "{name}({argument})"
        );
    }
    assert_eq!(outcome(&result, "nothing", vec![Value::u8(0)]), unit());
    assert_eq!(outcome(&result, "nothing", vec![Value::u8(1)]), unit());
    // Printed as written: `return value`, and `return` alone for `()`.
    let rust = print_module(result.session.erased());
    for line in [
        "return 1_u8",
        "return i",
        "return\n",
        "return n.wrapping_add(1_u8)",
    ] {
        assert!(rust.contains(line), "{line}\n{rust}");
    }
}

#[test]
fn evidence_owed_at_an_early_return_is_reported_at_the_return() {
    // The value of a `return` is checked against the result type where the
    // `return` stands: the fact of the branch fills the hole.
    let result = accepted(
        "fn clamp(x: u8) -> (out: u8, @(out <= 3)) { if x <= 3 { return (x, _); } else { } (3, _) }",
    );
    assert_eq!(result.holes.len(), 2);
    assert!(result.holes.iter().all(|hole| hole.solved));
    assert!(matches!(
        outcome(&result, "clamp", vec![Value::u8(2)]),
        Outcome::Value(Value::Tuple(fields)) if matches!(fields.as_slice(), [Value::Int(_, 2), _])
    ));
    // Not shown: the usual diagnostic, at the `return`, and the hole is
    // where the evidence is owed.
    let text = "fn clamp(x: u8) -> (out: u8, @(out <= 3)) { if x <= 9 { return (x, _); } else { } (3, _) }";
    let result = elaborated(text);
    assert!(!result.is_success());
    let diagnostic = &result.diagnostics[0];
    assert_eq!(diagnostic.code, "L0230");
    assert!(
        diagnostic.message.contains("cannot show `x <= 3`"),
        "{}",
        diagnostic.message
    );
    let primary = diagnostic
        .labels
        .iter()
        .find(|label| label.primary)
        .expect("a primary label");
    assert_eq!(&text[primary.span.range()], "return");
    assert_eq!(primary.message, "at this early return");
    let owed = diagnostic
        .labels
        .iter()
        .find(|label| label.message == "the evidence owed here")
        .expect("the hole is labelled");
    assert_eq!(&text[owed.span.range()], "_");
}

#[test]
fn the_never_type_coerces_to_any_type_in_each_position() {
    // `return`, `break`, `continue`, the forms that panic, and a call to a
    // function declared `-> !` produce no value, and stand where any type
    // is expected: as a `let`'s value, as an arm, as a tail.
    let result = accepted(
        "fn as_value(n: u8) -> u8 { let x: u8 = return n; x }
        fn as_arm(c: bool) -> u8 { if c { return 1 } else { 5 } }
        fn as_tail(n: u8) -> (out: u8, @(out == n)) { return (n, _) }
        fn spin() -> ! { loop {} }
        fn never_as_arm(n: u8) -> u8 { if n == 0 { spin() } else { n } }
        fn never_as_tail(n: u8) -> u8 { spin() }
        fn never_as_never(n: u8) -> ! { spin() }
        fn never_as_statement(n: u8) -> u8 { if n == 0 { spin(); } else { } n }
        enum Step { More(u8), Done }
        fn in_arm(step: Step) -> u8 { match step { Step::More(n) => return n, Step::Done => 0 } }
        fn breaking(n: u8) -> u8 {
            let mut i: u8 = 0;
            loop { i = i.wrapping_add(1); let x: u8 = if i == n { break } else { i }; if x == 200 { continue } else { } }
            i
        }",
    );
    for (name, argument, expected) in [
        ("as_value", 7, 7),
        ("never_as_arm", 3, 3),
        ("never_as_statement", 3, 3),
        ("breaking", 4, 4),
    ] {
        assert_eq!(
            outcome(&result, name, vec![Value::u8(argument)]),
            returned(expected),
            "{name}({argument})"
        );
    }
    assert_eq!(
        outcome(&result, "as_arm", vec![Value::Bool(true)]),
        returned(1)
    );
    assert_eq!(
        outcome(&result, "as_arm", vec![Value::Bool(false)]),
        returned(5)
    );
    assert!(matches!(
        outcome(&result, "as_tail", vec![Value::u8(4)]),
        Outcome::Value(Value::Tuple(fields)) if matches!(fields.as_slice(), [Value::Int(_, 4), _])
    ));
    // In the logic a function that never returns yields evidence of
    // `False`, and a call where a value is wanted is followed by the
    // point the evidence shows unreachable.
    let rust = print_module(result.session.erased());
    for line in [
        "let x: u8 = (return n);",
        "fn spin() -> Proved {",
        "spin();\n        unreachable!(\"shown never to be reached\")",
        "fn never_as_tail(n: u8) -> u8 {\n    spin();\n    unreachable!(\"shown never to be reached\")",
        "fn never_as_never(n: u8) -> Proved {\n    spin()\n}",
    ] {
        assert!(rust.contains(line), "{line}\n{rust}");
    }
}

#[test]
fn code_after_a_transfer_of_control_is_an_unreachable_warning() {
    // As rustc warns: on the first statement or the tail after an
    // expression statement of the never type, once per block. The file is
    // accepted, and what is unreachable is not elaborated or printed.
    let cases = [
        (
            "fn f(n: u8) -> u8 { return n; let m = n; m }",
            "unreachable statement",
            "let m = n;",
        ),
        (
            "fn f(n: u8) -> u8 { return n; n }",
            "unreachable expression",
            "n }",
        ),
        (
            "fn f(n: u8) -> u8 { loop { break; let m = n; } n }",
            "unreachable statement",
            "let m = n;",
        ),
        (
            "fn f(n: u8) -> u8 { let mut i: u8 = 0; while i < n { i = i.wrapping_add(1); continue; i = 0; } i }",
            "unreachable statement",
            "i = 0;",
        ),
        (
            "fn f(n: u8) -> u8 { todo!(); n }",
            "unreachable expression",
            "n }",
        ),
        (
            "fn f(n: u8) -> u8 { panic!(\"x\"); n }",
            "unreachable expression",
            "n }",
        ),
        (
            "fn spin() -> ! { loop {} } fn f(n: u8) -> u8 { spin(); n }",
            "unreachable expression",
            "n }",
        ),
    ];
    for (text, message, at) in cases {
        let result = elaborated(text);
        assert!(result.is_success(), "{text}");
        assert_eq!(result.diagnostics.len(), 1, "{text}");
        let warning = &result.diagnostics[0];
        assert!(!warning.is_error(), "{text}");
        assert_eq!(warning.code, "L0247", "{text}");
        assert_eq!(warning.message, message, "{text}");
        let primary = warning
            .labels
            .iter()
            .find(|label| label.primary)
            .expect("a primary label");
        assert!(text[primary.span.start..].starts_with(at), "{text}");
        let context = warning
            .labels
            .iter()
            .find(|label| !label.primary)
            .expect("the expression that leaves is labelled");
        assert_eq!(
            context.message,
            "any code following this expression is unreachable"
        );
        assert_eq!(check_module(result.session.erased()), Ok(()));
    }
    // The dead code is gone from the Rust, which begins with the transfer.
    let result = elaborated("fn f(n: u8) -> u8 { return n; let m = n; m }");
    let rust = print_module(result.session.erased());
    assert!(
        rust.contains("fn f(n: u8) -> u8 {\n    return n\n}"),
        "{rust}"
    );
    // Nothing follows a transfer that is not a statement: `let x = return;`
    // binds, as E10 chose for a `let` bound to a panic.
    let result = elaborated("fn f(n: u8) -> u8 { let x: u8 = return n; x }");
    assert!(result.diagnostics.is_empty());
}

#[test]
fn a_function_declared_never_never_returns() {
    // Its body ends in a panic, a `loop` without `break`, or a call to
    // such a function; in the logic it yields evidence of `False`.
    let result = accepted(
        "fn spin() -> ! { loop {} }
        fn stop() -> ! { panic!(\"stop\") }
        fn either(c: bool) -> ! { if c { spin() } else { stop() } }
        fn uses(n: u8) -> u8 { if n == 0 { stop() } else { n } }",
    );
    assert_eq!(
        outcome(&result, "uses", vec![Value::u8(0)]),
        panicked("stop")
    );
    assert_eq!(outcome(&result, "uses", vec![Value::u8(2)]), returned(2));
    for (text, code, fragment) in [
        (
            "fn f(n: u8) -> ! { if n == 0 { panic!() } else { } }",
            "L0220",
            "`f` is declared `-> !`, and its body can reach its end",
        ),
        (
            "fn f(n: u8) -> ! { n }",
            "L0220",
            "`f` is declared `-> !`, and its body can reach its end",
        ),
        (
            "fn f(n: u8) -> ! { if n == 0 { return; } else { loop {} } }",
            "L0220",
            "`return` in `f`, which is declared `-> !`",
        ),
        (
            "fn f(n: u8) -> u8 { let x: ! = panic!(); n }",
            "L0290",
            "the never type `!` stands only as the result type",
        ),
        (
            "fn f(n: u8) -> u8 { if n == 0 { return; } else { } n }",
            "L0220",
            "this `return` carries no value, and `f` returns `u8`",
        ),
        (
            "fn f(n: u8) -> Prop { prop!(return 1) }",
            "L0215",
            "`return` cannot appear in a proposition",
        ),
        (
            "const C: u8 = return 1;",
            "L0215",
            "`return` cannot appear in the value of a constant",
        ),
    ] {
        let (codes, full) = rejected(text);
        assert_eq!(codes, [code], "{text}: {full}");
        assert!(full.contains(fragment), "{text}: {full}");
    }
    // A function of the logic with a `return` in its body is not a kernel
    // term: it is elaborated again as an ordinary function with its
    // promises (LOC-193).
    let result = accepted(
        "#[terminates] #[no_panic] #[no_io] fn f(n: u8) -> u8 { if n == 0 { return 1 } else { n } }",
    );
    assert_eq!(outcome(&result, "f", vec![Value::u8(0)]), returned(1));
}
