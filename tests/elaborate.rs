//! Source to checked program: parsing, elaboration with holes, kernel
//! acceptance, interpretation, and generated Rust, from `.lc` text.

use std::path::PathBuf;
use std::process::Command;

use locus::elab::{Elaborated, elaborate};
use locus::erased::{Interpreter, Value, check_module, print_module};
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
    let values = arguments.iter().copied().map(Value::U8).collect();
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
    // Every `_` was filled by the search and accepted by the kernel.
    assert_eq!(result.holes.len(), 8);
    assert!(result.holes.iter().all(|hole| hole.solved));
}

#[test]
fn the_generated_rust_reads_like_the_source_and_agrees_with_the_interpreter() {
    let result = accepted(LOCK);
    let module = result.session.erased();
    let mut source = print_module(module);
    for expected in [
        "pub struct Lock {",
        "pub fn step(lock: Lock, bounded: Proved, event: Event) -> (Lock, Proved) {",
        "    match event {",
        "            if lock.failures < 3 {",
        "                (Lock { failures: lock.failures.wrapping_add(1), open: false }, Proved)",
        "    for attempt in 0..attempts {",
        "        let (next, still_bounded) = step(lock, bounded, event_at(attempt, correct));",
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
                (Lock { failures: lock.failures.wrapping_add(1), open: false }, _)
            } else {
                (Lock { failures: lock.failures, open: false }, _)
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
            "after computing, the claim is `lock.failures.wrapping_add(1) <= 3`",
            "it fails when `lock.failures` is 3, which the facts known here allow",
            "known here: `lock.failures <= 3`",
        ]
    );
    // Nothing unverified was emitted.
    assert!(result.function("step").is_none());
    assert!(result.function("run").is_none());
    assert!(result.function("event_at").is_some());
}

// Routine rearrangements of a program that checks need no proof repair.

const INCREMENT: &str = "fn increment(n: u8) -> (out: u8, @[out == n.wrapping_add(1)]) {
    let out = n.wrapping_add(1);
    (out, _)
}";

#[test]
fn introducing_a_local_needs_no_proof_repair() {
    let result = accepted(
        "fn increment(n: u8) -> (out: u8, @[out == n.wrapping_add(1)]) {
            let one = 1;
            let sum = n.wrapping_add(one);
            let out = sum;
            (out, _)
        }",
    );
    assert_eq!(call(&result, "increment", &[255]), "(0, Proved)");
    let inline = accepted(
        "fn increment(n: u8) -> (out: u8, @[out == n.wrapping_add(1)]) { (n.wrapping_add(1), _) }",
    );
    assert_eq!(call(&inline, "increment", &[7]), "(8, Proved)");
}

#[test]
fn destructuring_a_result_keeps_its_evidence_usable() {
    let result = accepted(&format!(
        "{INCREMENT}
        fn twice(n: u8) -> (out: u8, @[out == n.wrapping_add(1).wrapping_add(1)]) {{
            let (first, first_is) = increment(n);
            let (second, second_is) = increment(first);
            (second, _)
        }}
        fn twice_without_names(n: u8) -> (out: u8, @[out == n.wrapping_add(1).wrapping_add(1)]) {{
            let once = increment(n);
            let again = increment(once.0);
            (again.0, _)
        }}"
    ));
    assert_eq!(call(&result, "twice", &[254]), "(0, Proved)");
    assert_eq!(call(&result, "twice_without_names", &[1]), "(3, Proved)");
}

#[test]
fn extracting_a_helper_needs_no_proof_repair() {
    // `step` of the lock, with the saturating increment moved into a helper
    // whose result says what the caller needs.
    let result = accepted(
        "math fn within_limit(failures: u8) -> Prop { [failures <= 3] }
        fn bump(failures: u8, bounded: @within_limit(failures)) -> (next: u8, @within_limit(next)) {
            if failures < 3 { (failures.wrapping_add(1), _) } else { (failures, _) }
        }
        fn record(failures: u8, bounded: @within_limit(failures), wrong: u8) -> (next: u8, @within_limit(next)) {
            if wrong == 0 {
                (0, _)
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
            vec![Value::U8(3), Value::Proved, Value::U8(1)],
        )
        .unwrap();
    assert_eq!(value.debug(module), "(3, Proved)");
}

#[test]
fn evidence_about_a_projection_serves_for_the_name_bound_to_it() {
    // `still` is evidence about `bump(...).0`; it is accepted as evidence
    // about `next`, which is that projection.
    let result = accepted(
        "fn bump(n: u8, small: @[n < 10]) -> (out: u8, @[out <= 10]) { (n.wrapping_add(1), _) }
        fn use_it(n: u8, small: @[n < 10]) -> (out: u8, @[out <= 10]) {
            let (next, still) = bump(n, small);
            let copy = next;
            (copy, still)
        }",
    );
    assert_eq!(result.holes.iter().filter(|hole| hole.solved).count(), 2);
}

#[test]
fn a_math_fn_runs_and_is_usable_in_claims() {
    let result = accepted(
        "math fn double_step(n: u8) -> u8 { n.wrapping_add(2) }
        fn advance(n: u8) -> (out: u8, @[out == double_step(n)]) {
            (n.wrapping_add(1).wrapping_add(1), _)
        }",
    );
    assert_eq!(call(&result, "double_step", &[254]), "0");
    assert_eq!(call(&result, "advance", &[3]), "(5, Proved)");
}

#[test]
fn a_loop_carries_its_invariant_as_state() {
    let result = accepted(
        "fn walk(limit: u8) -> (out: u8, @[out <= limit]) {
            loop (i: u8 = 0, bound: @[i <= limit] = _) -> (out: u8, @[out <= limit]) {
                if i == limit {
                    break (i, bound)
                } else {
                    continue(limit, _)
                }
            }
        }",
    );
    assert_eq!(call(&result, "walk", &[9]), "(9, Proved)");
}

#[test]
fn boolean_connectives_short_circuit_and_feed_branch_facts() {
    let result = accepted(
        "fn clamp(n: u8) -> (out: u8, @[out <= 9]) {
            if 3 <= n && n <= 9 { (n, _) } else { (9, _) }
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
    let cases: [(&str, &str, &str); 14] = [
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
            "fn f(n: u8) -> u8 { n } math fn g(n: u8) -> u8 { f(n) }",
            "L0209",
            "may fail to return",
        ),
        (
            "enum E { A, B } fn f(e: E) -> u8 { match e { E::A => 0 } }",
            "L0213",
            "no arm handles `E::B`",
        ),
        (
            "fn f(n: u8) -> u8 { for i in 0..n (a: u8 = 0) { break a } }",
            "L0218",
            "has no `break`",
        ),
        (
            "math fn f(n: u8) -> u8 { loop () -> u8 { break n } }",
            "L0215",
            "may run forever",
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
            "fn f(flag: bool) -> @[flag] { _ }",
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
    let (codes, full) = rejected("fn f(n: u8) -> @[n.wrapping_add(1) != 0] { _ }");
    assert_eq!(codes, ["L0230"]);
    assert!(full.contains("it fails when `n` is 255"), "{full}");
    let (_, full) = rejected("fn f(n: u8, m: u8) -> @[n <= m] { _ }");
    assert!(full.contains("cannot show `n <= m`"), "{full}");
    assert!(full.contains("nothing known here"), "{full}");
}

#[test]
fn a_reversed_range_is_rejected_for_want_of_evidence() {
    let (codes, full) = rejected("fn f(n: u8) -> () { for i in 5..n () { continue() } }");
    assert_eq!(codes, ["L0230"]);
    assert!(full.contains("cannot show `5 <= n`"), "{full}");
    assert!(full.contains("it fails when `n` is 0"), "{full}");
    accepted(
        "fn f(n: u8) -> () {
            if 5 <= n { for i in 5..n () { continue() } } else { () }
        }",
    );
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
        math fn small_prime_is_small(n: u8, h: @SmallPrime(n)) -> @[n <= 7] {
            match h {
                SmallPrime::Two => _,
                SmallPrime::Three => _,
                SmallPrime::Five => _,
                SmallPrime::Seven => _,
            }
        }
        math fn seven_is(h: @SmallPrime(7)) -> @[7 <= 7] { small_prime_is_small(7, h) }
        math fn five() -> @SmallPrime(5) { SmallPrime::Five }",
    );
    // The equations are what make the arms provable.
    let (codes, full) = rejected(
        "prop SmallPrime(n: u8) { Two: @SmallPrime(2), Seven: @SmallPrime(7) }
        math fn too_small(n: u8, h: @SmallPrime(n)) -> @[n <= 6] {
            match h { SmallPrime::Two => _, SmallPrime::Seven => _ }
        }",
    );
    assert_eq!(codes, ["L0230"]);
    assert!(full.contains("cannot show `n <= 6`"), "{full}");
    assert!(
        full.contains("after computing, the claim is `7 <= 6`"),
        "{full}"
    );
}

#[test]
fn connectives_are_built_and_taken_apart_by_their_constructors() {
    accepted(
        "math fn swap(p: Prop, q: Prop, h: @[p || q]) -> @[q || p] {
            match h {
                Or::Left(hp) => Or::Right(hp),
                Or::Right(hq) => Or::Left(hq),
            }
        }
        math fn both(p: Prop, q: Prop, hp: @p, hq: @q) -> @[q && p] { And::Intro(hq, hp) }
        math fn first(p: Prop, q: Prop, h: @[p && q]) -> @p {
            match h { And::Intro(hp, _) => hp }
        }
        math fn anything(p: Prop, h: @[false]) -> @p { match h {} }
        fn unreachable(n: u8, h: @[false]) -> u8 { match h {} }",
    );
    // A hole does the same within its budget.
    accepted(
        "math fn swap(p: Prop, q: Prop, h: @[p && q]) -> @[q && p] { _ }
        math fn weaken(p: Prop, q: Prop, hq: @q) -> @[p || q] { _ }
        math fn curry(p: Prop, q: Prop, hq: @q) -> @[p => q && q] { _ }
        math fn modus(p: Prop, q: Prop, hp: @p, h: @[!p]) -> @[false] { _ }
        math fn ordered() -> @[forall (x: u8) { x <= 3 => x < 4 }] { _ }",
    );
}

#[test]
fn evidence_cannot_choose_a_value() {
    let (codes, full) = rejected(
        "fn pick(p: Prop, q: Prop, h: @[p || q]) -> u8 {
            match h { Or::Left(_) => 0, Or::Right(_) => 1 }
        }",
    );
    assert_eq!(codes, ["L0227"]);
    assert!(full.contains("only to produce other evidence"), "{full}");
}

#[test]
fn a_math_fn_is_evidence_of_its_general_claim_and_evidence_is_applied() {
    accepted(
        "math fn self_equal(x: u8) -> @[x == x] { _ }
        math fn all_self_equal() -> @[forall (x: u8) { x == x }] { self_equal }

        math fn at_most_nine_helper(limit: u8, h: @[limit <= 9], x: u8, hx: @[x <= limit]) -> @[x <= 9] {
            u8_le_trans(x, limit, 9, hx, h)
        }
        math fn at_most_nine(limit: u8, h: @[limit <= 9]) -> @[forall (x: u8) { x <= limit => x <= 9 }] {
            let general: @[forall (l: u8) { l <= 9 => forall (x: u8) { x <= l => x <= 9 } }] =
                at_most_nine_helper;
            general(limit)(h)
        }
        math fn use_it(h: @[forall (x: u8) { x <= 255 }], n: u8) -> @[n <= 255] { h(n) }",
    );
    let (codes, full) = rejected("math fn f(h: @[1 == 1], n: u8) -> @[1 == 1] { h(n) }");
    assert_eq!(codes, ["L0228"]);
    assert!(full.contains("takes no argument"), "{full}");
}

#[test]
fn rewrite_unfold_and_fold_are_the_explicit_forms() {
    accepted(
        "math fn nonzero(x: u8) -> Prop { [x != 0] }
        math fn use_nonzero(n: u8, h: @nonzero(n)) -> @[n != 0] { unfold(nonzero, h) }
        math fn make_nonzero(n: u8, h: @[n != 0]) -> @nonzero(n) { fold(nonzero, h) }
        math fn moved(a: u8, b: u8, same: @[a == b], small: @[a <= 9]) -> @[b <= 9] {
            rewrite(same, small)
        }",
    );
    let (codes, full) = rejected(
        "math fn nonzero(x: u8) -> Prop { [x != 0] }
        math fn f(n: u8, h: @[n != 0]) -> @nonzero(n) { let folded = fold(nonzero, h); folded }",
    );
    assert_eq!(codes, ["L0229"]);
    assert!(full.contains("needs to know the claim"), "{full}");
}

#[test]
fn a_constant_is_used_by_name() {
    let result = accepted(
        "const LIMIT: u8 = 3;
        const limit_is_small: Prop = [LIMIT <= 9];
        math fn known() -> @limit_is_small { _ }
        fn clamp(n: u8) -> (out: u8, @[out <= LIMIT]) {
            if n <= LIMIT { (n, _) } else { (LIMIT, _) }
        }",
    );
    assert_eq!(call(&result, "clamp", &[2]), "(2, Proved)");
    assert_eq!(call(&result, "clamp", &[200]), "(3, Proved)");
}

#[test]
fn a_constructor_needs_to_know_what_it_proves() {
    let (codes, _) =
        rejected("math fn f(p: Prop, q: Prop, hp: @p) -> () { let h = Or::Left(hp); () }");
    assert_eq!(codes, ["L0226"]);
    accepted("math fn f(p: Prop, q: Prop, hp: @p) -> () { let h: @[p || q] = Or::Left(hp); () }");
}
