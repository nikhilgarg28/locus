//! Named proposition arms, their explicit evidence slots, and proof elimination.
use locus::elab::{Elaborated, Options, elaborate_with_options};
use locus::erased::{Interpreter, Value, check_module};
use locus::parser::parse;
use locus::source::SourceMap;

fn elaborate(text: &str) -> Elaborated {
    let mut sources = SourceMap::default();
    let id = sources.add("named-props.lc", text);
    let source = sources.get(id);
    let parsed = parse(source);
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let mut options = Options::default();
    for name in ["logical-split", "named-props"] {
        let feature = locus::preview::Feature::parse(name).unwrap();
        if feature.status() == locus::preview::Status::Preview {
            options.previews.enable(name).unwrap();
        }
    }
    elaborate_with_options(source, &parsed.program, &options)
}

fn accepted(text: &str) -> Elaborated {
    let result = elaborate(text);
    assert!(result.is_success(), "{:#?}", result.diagnostics);
    assert_eq!(check_module(result.session.erased()), Ok(()));
    result
}

fn rejected(text: &str, code: &str, message: &str) {
    let result = elaborate(text);
    assert!(!result.is_success(), "accepted {text}");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == code && d.message.contains(message)),
        "{:#?}",
        result.diagnostics
    );
}

#[test]
fn named_unit_arm_closes_and_opens_a_claim_in_an_ordinary_function() {
    accepted(
        "prop Within(n: Int) { Bounds => { prop!(n <= 3) } }
        fn close(n: u8, h: @(n as Int <= 3)) -> @Within(n as Int) { Within::Bounds @ h }
        fn open(n: u8, h: @Within(n as Int)) -> @(n as Int <= 3) {
            let Within::Bounds @ small = h;
            small
        }",
    );
}

#[test]
#[doc = "spec: 1.6:6"]
fn witnesses_and_named_fields_are_checked_separately_from_evidence() {
    accepted(
        "prop Contains(n: Int) {
            At(index: Int) => { prop!(index == n) }
            Named { index: Int } => { prop!(index == n) }
        }
        fn tuple() -> @Contains(3) { Contains::At(3) @ prove!(3 == 3) }
        fn named() -> @Contains(3) { Contains::Named { index: 3 } @ prove!(3 == 3) }
        fn inspect(h: @Contains(3)) -> @(true) {
            match h {
                Contains::At(index) @ found => True::Intro,
                Contains::Named { index: renamed } @ found => True::Intro
            }
        }",
    );
}

#[test]
fn whole_value_patterns_alias_the_original_evidence() {
    accepted(
        "prop Bound { Yes => { prop!(true) } }
        fn reopen(h: @Bound) -> @Bound { let whole @ (Bound::Yes @ body) = h; whole }
        fn rematch(h: @Bound) -> @Bound { match h { whole @ (Bound::Yes @ body) => whole } }",
    );
}

#[test]
fn computed_arm_bodies_have_locals_and_logical_calls() {
    accepted("logic fn same(x: Int, y: Int) -> Prop { prop!(x == y) }
        prop IsSuccessor(n: Int, next: Int) { ByDefinition => { let expected = n + 1; same(expected, next) } }
        fn make() -> @IsSuccessor(3, 4) {
            let equation = prove!(4 == 4);
            let folded: @same(4, 4) = fold!(same, equation);
            IsSuccessor::ByDefinition @ folded
        }");
}

#[test]
fn ordinary_evidence_producers_run_exactly_once_before_erasure() {
    let result = accepted(
        "prop Done { Yes => { prop!(true) } }
        fn mutate(x: &mut u8) -> @(true) { x = x.wrapping_add(1); True::Intro }
        fn run() -> u8 {
            let mut x: u8 = 0;
            let done: @Done = Done::Yes @ mutate(&mut x);
            let Done::Yes @ body = done;
            x
        }",
    );
    let module = result.session.erased();
    let function = result.function("run").unwrap();
    let value = Interpreter::new(module, 1000)
        .call(function, Vec::<Value>::new())
        .unwrap();
    assert_eq!(value.debug(module), "1");
}

#[test]
fn missing_witness_and_missing_evidence_are_distinct_errors() {
    rejected(
        "prop P { At(index: Int) => { prop!(true) } } fn f() -> @P { P::At @ True::Intro }",
        "L0275",
        "requires 1 witness",
    );
    rejected(
        "prop P { At(index: Int) => { prop!(true) } } fn f() -> @P { P::At(3) }",
        "L0276",
        "evidence outside",
    );
}

#[test]
fn evidence_for_a_different_arm_body_is_rejected() {
    let result = elaborate(
        "prop Positive(n: Int) { Check => { prop!(n > 0) } } fn f() -> @Positive(0) { Positive::Check @ prove!(0 == 0) }",
    );
    assert!(!result.is_success());
    assert!(
        !result.diagnostics.iter().any(|d| d.code == "L0300"),
        "{:#?}",
        result.diagnostics
    );
}

#[test]
fn propositions_are_opened_by_matching_not_unfolding() {
    rejected(
        "prop P { Yes => { prop!(true) } } fn f(h: @P) -> @(true) { unfold!(P, h) }",
        "L0277",
        "opened by matching",
    );
}

#[test]
#[doc = "spec: 1.18:4, 1.26:5"]
fn evidence_cannot_select_runtime_data_or_export_arbitrary_witnesses() {
    rejected(
        "prop P { A => { prop!(true) } B => { prop!(true) } }
        fn f(h: @P) -> u8 { match h { P::A @ a => 1, P::B @ b => 2 } }",
        "L0227",
        "only to produce other evidence",
    );
    rejected(
        "prop Has { At(witness: Int) => { prop!(true) } }
        fn f(h: @Has) -> Int { let Has::At(witness) @ body = h; witness }",
        "L0278",
        "cannot extract arbitrary witnesses",
    );
}

#[test]
fn rewriting_a_logical_equality_rewrites_values_not_the_boolean_test() {
    accepted(
        "logic fn transport(x: Int, y: Int, equal: @(x == y), small: @(x <= 3)) -> @(y <= 3) {
        rewrite!(equal, small)
    }",
    );
}

#[test]
fn legacy_constructor_evidence_fix_is_applied_and_rechecked() {
    let text = "prop P { Yes => { prop!(true) } } fn f() -> @P { P::Yes(True::Intro) }";
    let result = elaborate(text);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|d| d.code == "L0276")
        .unwrap();
    let fix = &diagnostic.suggestions[0];
    let mut fixed = text.to_owned();
    fixed.replace_range(fix.span.range(), &fix.replacement);
    accepted(&fixed);
}

#[test]
fn ordinary_proof_match_scrutinee_still_runs() {
    let result = accepted(
        "prop Done { Yes => { prop!(true) } }
        fn mutate(x: &mut u8) -> @Done { x = x.wrapping_add(1); Done::Yes @ True::Intro }
        fn run() -> u8 {
            let mut x: u8 = 0;
            let h: @(true) = match mutate(&mut x) { Done::Yes @ h => h };
            x
        }",
    );
    let module = result.session.erased();
    let value = Interpreter::new(module, 1000)
        .call(result.function("run").unwrap(), Vec::new())
        .unwrap();
    assert_eq!(value.debug(module), "1");
}

#[path = "common/corpus.rs"]
mod runner;

#[test]
fn reconciliation_rejections_pin_codes_locations_and_necessary_preview_flags() {
    for name in [
        "evidence_at_chain",
        "evidence_pattern_left",
        "legacy_prop_arm",
        "prop_missing_witness",
        "prop_missing_evidence",
        "prop_unfold",
        "prop_witness_escape",
    ] {
        let path = format!("tests/corpus/reject/{name}.lc");
        let text =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(&path))
                .unwrap();
        let examined = runner::examine(&path, &text);
        assert!(examined.failures.is_empty(), "{:#?}", examined.failures);
    }
}

#[test]
fn explicit_proof_transformations_preserve_eager_runtime_operands() {
    let source = "logic fn good() -> Prop { prop!(true) }
        fn plain(x: &mut u8) -> @(true) { x = x.wrapping_add(1); True::Intro }
        fn wrapped(x: &mut u8) -> @good() { x = x.wrapping_add(2); fold!(good, True::Intro) }
        fn equation(x: &mut u8) -> @(1 + 0 == 1) { x = x.wrapping_add(4); prove!(1 + 0 == 1) }
        fn claim(x: &mut u8) -> @(1 + 0 == 1 + 0) { x = x.wrapping_add(8); prove!(1 + 0 == 1 + 0) }
        fn run() -> u8 {
            let mut x: u8 = 0;
            let folded: @good() = fold!(good, plain(&mut x));
            let opened: @(true) = unfold!(good, wrapped(&mut x));
            let rewritten: @(1 == 1) = rewrite!(equation(&mut x), claim(&mut x));
            x
        }";
    let checked = accepted(source);
    let module = checked.session.erased();
    let function = checked.function("run").unwrap();
    let erased = Interpreter::new(module, 10000)
        .call(function, vec![])
        .unwrap();
    let check = locus::exec::CheckInterpreter::new(checked.session.program(), 10000)
        .with_lending(checked.session.lending())
        .call(function, vec![])
        .unwrap();
    assert_eq!(erased, check);
    assert_eq!(erased.debug(module), "15");
    let directory =
        std::env::temp_dir().join(format!("locus-proof-effects-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let rust = format!(
        "{}\nfn main() {{ println!(\"{{}}\", run()); }}",
        locus::erased::print_module(module)
    );
    std::fs::write(directory.join("main.rs"), rust).unwrap();
    for overflow in ["yes", "no"] {
        let compiled = std::process::Command::new("rustc")
            .args(["--edition=2024", "-Dwarnings", "-C"])
            .arg(format!("overflow-checks={overflow}"))
            .arg(directory.join("main.rs"))
            .arg("-o")
            .arg(directory.join("run"))
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let output = std::process::Command::new(directory.join("run"))
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "15");
    }
    std::fs::remove_dir_all(directory).unwrap();
}
