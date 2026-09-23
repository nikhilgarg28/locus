//! User-defined models are checked observations, never unchecked axioms.
use locus::elab::{Elaborated, Options, elaborate_with_options};
use locus::erased::{Interpreter, check_module};
use locus::parser::parse;
use locus::source::SourceMap;
fn check(text: &str) -> Elaborated {
    let mut map = SourceMap::default();
    let id = map.add("models.lc", text);
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
fn run(text: &str, expected: &str) {
    let result = accepted(text);
    let module = result.session.erased();
    let value = Interpreter::new(module, 10000)
        .call(result.function("run").unwrap(), vec![])
        .unwrap();
    assert_eq!(value.debug(module), expected);
}
const POINT: &str = "struct Point { x: u8 }
#[derive(Logical)] struct Position { x: Int }
impl Model<Point> for Position {
    logic fn model(source: &Point) -> Self { Self { x: source.x as Int } }
}";
#[test]
#[doc = "spec: 1.25:5, 1.3:4"]
fn observed_struct_is_not_moved_and_snapshot_keeps_its_version() {
    run(
        &format!(
            "{POINT} fn run() -> u8 {{ let mut point = Point {{ x: 3 }}; let before = point as Position; point.x = 4; let h = prove!(before.x == 3); point.x }}"
        ),
        "4",
    );
}
#[test]
#[doc = "spec: 1.25:5"]
fn multiple_destinations_and_composed_models_are_resolved_by_type() {
    accepted(&format!("{POINT}
        impl Model<Point> for Int {{ logic fn model(source: &Point) -> Self {{ let position = source as Position; position.x }} }}
        fn run(point: Point) -> Int {{ point as Int }}"));
}
#[test]
fn casts_use_an_active_mutable_referent_and_do_not_retain_the_loan() {
    run(&format!("{POINT}
        fn observe(point: &mut Point) -> Int {{ let snapshot = point as Position; point.x = 9; snapshot.x }}
        fn run() -> u8 {{ let mut point = Point {{ x: 3 }}; let before = observe(&mut point); let again = point as Position; point.x }}"), "9");
}
#[test]
fn eager_runtime_source_computation_runs_once() {
    run(
        &format!(
            "{POINT}
        fn make(n: &mut u8) -> Point {{ n = n.wrapping_add(1); Point {{ x: n }} }}
        fn run() -> u8 {{ let mut n: u8 = 0; let snapshot = make(&mut n) as Position; n }}"
        ),
        "1",
    );
}
#[test]
fn model_cannot_call_an_ordinary_function() {
    let result = check(
        "struct Point { x: u8 } fn getter(p: &Point) -> u8 { p.x } impl Model<Point> for Int { logic fn model(source: &Point) -> Self { getter(&source) as Int } }",
    );
    assert!(
        result.diagnostics.iter().any(|d| d.code == "L0209"),
        "{:#?}",
        result.diagnostics
    );
}
#[test]
#[doc = "spec: 1.25:5"]
fn duplicate_pairs_missing_models_and_wrong_signatures_are_rejected() {
    for text in [
        "struct Point { x: u8 } impl Model<Point> for Int { logic fn model(source: &Point) -> Self { source.x as Int } } impl Model<Point> for Int { logic fn model(source: &Point) -> Self { 0 } }",
        "#[derive(Logical)] struct Data { n: Int } fn f(x: u8) -> Data { x as Data }",
        "struct Point { x: u8 } impl Model<Point> for Int { fn model(source: &Point) -> Int { 0 } }",
        "struct Point { x: u8 } impl Model<Point> for Int { logic fn model(source: Point) -> Self { 0 } }",
    ] {
        let result = check(text);
        assert!(
            result.diagnostics.iter().any(|d| d.code == "L0282"),
            "{text}\n{:#?}",
            result.diagnostics
        );
    }
}
#[test]
fn model_declaration_order_does_not_change_cast_resolution() {
    accepted(
        "struct Point { x: u8 } fn use_model(point: Point) -> Int { point as Int } impl Model<Point> for Int { logic fn model(source: &Point) -> Self { source.x as Int } }",
    );
}
#[test]
fn generic_model_destination_and_primitive_source_preserve_their_types() {
    accepted("#[derive(Logical)] struct View<T: Logical> { value: T }
        impl Model<u8> for View<Int> { logic fn model(source: &u8) -> Self { Self { value: source as Int } } }
        fn observe(n: u8) -> View<Int> { n as View<Int> }");
}

#[test]
fn fixed_array_models_keep_source_shape_and_do_not_apply_to_other_lengths() {
    let source = "impl Model<[u8; 1]> for Int { logic fn model(source: &[u8; 1]) -> Self { 1 } }
        impl Model<[u8; 2]> for Int { logic fn model(source: &[u8; 2]) -> Self { 2 } }
        fn observe() -> @(true) {
            let a: [u8; 1] = [3]; let b: [u8; 2] = [4, 5];
            let one = a as Int; let two = b as Int;
            let h = prove!(one == 1); let h = prove!(two == 2); True::Intro
        }";
    let result = heap_check(source);
    assert!(result.is_success(), "{:#?}", result.diagnostics);
    let result = heap_check(
        "impl Model<[u8; 1]> for Int { logic fn model(source: &[u8; 1]) -> Self { 1 } }
        fn bad(values: &[u8; 2]) -> Int { values as Int }",
    );
    assert!(
        result.diagnostics.iter().any(|d| d.code == "L0282"),
        "{:#?}",
        result.diagnostics
    );
}
#[test]
fn slice_and_shape_specific_models_cannot_overlap() {
    for (first, second) in [("[u8]", "[u8; 2]"), ("Vec<u8>", "[u8]")] {
        let result = heap_check(&format!(
            "impl Model<{first}> for Int {{ logic fn model(source: &{first}) -> Self {{ 1 }} }}
            impl Model<{second}> for Int {{ logic fn model(source: &{second}) -> Self {{ 2 }} }}"
        ));
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.code == "L0282" && d.message.contains("overlapping")),
            "{:#?}",
            result.diagnostics
        );
    }
}
fn heap_check(text: &str) -> Elaborated {
    let mut map = SourceMap::default();
    let id = map.add("model_shapes.lc", text);
    let source = map.get(id);
    let parsed = parse(source);
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
    let mut options = Options::default();
    for feature in [
        locus::preview::Feature::LogicalData,
        locus::preview::Feature::HeapViews,
    ] {
        if feature.status() == locus::preview::Status::Preview {
            options.previews.enable(feature.name()).unwrap();
        }
    }
    elaborate_with_options(source, &parsed.program, &options)
}
#[test]
fn model_definition_selectors_fold_and_unfold_checked_observations() {
    accepted(&format!(
        "{POINT}
        logic fn relation(point: &Point) -> @((point as Position).x == point.x as Int) {{
            fold!(point as Position, prove!(point.x as Int == point.x as Int))
        }}
        logic fn open(point: &Point, h: @((point as Position).x == 3)) -> @(point.x as Int == 3) {{
            unfold!(point as Position, h)
        }}"
    ));
}
#[test]
fn model_selectors_reject_wrong_observation_unknown_model_and_runtime_effects() {
    for code in [
        "logic fn wrong(a: &Point, b: &Point, h: @(b.x as Int == 3)) -> @((b as Position).x == 3) { fold!(a as Position, h) }",
        "logic fn missing(value: u8) -> @(value == value) { fold!(value as Position, prove!(value == value)) }",
        "fn make(n: &mut u8) -> Point { n = n.wrapping_add(1); Point { x: n } } fn effect(n: &mut u8) -> @(true) { fold!(make(&mut n) as Position, True::Intro) }",
    ] {
        let checked = check(&format!("{POINT} {code}"));
        assert!(!checked.is_success(), "{code}");
        assert!(
            checked
                .diagnostics
                .iter()
                .any(|d| matches!(d.code, "L0229" | "L0282" | "L0209")),
            "{:#?}",
            checked.diagnostics
        );
    }
}

#[test]
fn eager_runtime_arguments_still_move_inside_erased_observations() {
    let cases = [
        "let observed = consume(point) as Int;",
        "let observed = logical_identity(consume(point) as Int);",
        "let observed = Box::new(point) as Int;",
        "let observed = Vec::from([point]) as Int;",
        "let observed = [point] as Int;",
    ];
    for statement in cases {
        let source = format!("struct Point {{ x: u8 }}
            fn consume(point: Point) -> u8 {{ point.x }}
            logic fn logical_identity(value: Int) -> Int {{ value }}
            impl Model<Box<Point>> for Int {{ logic fn model(source: &Box<Point>) -> Self {{ 1 }} }}
            impl Model<[Point]> for Int {{ logic fn model(source: &[Point]) -> Self {{ 1 }} }}
            fn bad() -> u8 {{ let point = Point {{ x: 3 }}; {statement} let stale = prove!(point.x == 3); point.x }}");
        let result = heap_check(&source);
        assert!(!result.is_success(), "{statement}");
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| matches!(d.code, "L0240" | "L0241")),
            "{statement}: {:#?}",
            result.diagnostics
        );
    }
}

#[test]
fn pure_model_observations_keep_noncopy_sources_available() {
    run(
        &format!(
            "{POINT} fn run() -> u8 {{ let point = Point {{ x: 3 }}; let seen = point as Position; point.x }}"
        ),
        "3",
    );
}

#[test]
fn temporary_method_receivers_respect_the_selected_execution_mode() {
    let definitions = "struct Point { x: u8 }
        impl Point {
            fn consume(self) -> u8 { self.x }
            logic fn inspect(self) -> Int { self.x as Int }
        }";
    let rejected = check(&format!(
        "{definitions} fn bad() -> u8 {{
        let point = Point {{ x: 3 }};
        let observed = ({{ point }}).consume() as Int;
        point.x
    }}"
    ));
    assert!(
        rejected.diagnostics.iter().any(|d| d.code == "L0240"),
        "{:#?}",
        rejected.diagnostics
    );
    run(
        &format!(
            "{definitions} fn run() -> u8 {{
        let point = Point {{ x: 3 }};
        let observed = ({{ point }}).inspect();
        point.x
    }}"
        ),
        "3",
    );
    run(
        &format!(
            "{definitions}
        fn touch(counter: &mut u8) -> () {{ counter = counter.wrapping_add(1); }}
        fn run() -> u8 {{
            let mut counter: u8 = 0;
            let point = Point {{ x: 3 }};
            let observed = ({{ touch(&mut counter); point }}).consume() as Int;
            counter
        }}"
        ),
        "1",
    );
}

#[test]
fn receiver_type_probes_do_not_change_proof_cache_or_generated_execution() {
    use locus::elab::elaborate_with_store;
    use locus::store::ProofStore;
    let source = "struct Point { x: u8 }
        impl Point { fn consume(self) -> u8 { self.x } }
        fn touch(counter: &mut u8) -> () { counter = counter.wrapping_add(1); }
        fn run() -> u8 {
            let mut counter: u8 = 0;
            let point = Point { x: 3 };
            let observed = ({ let h = prove!(1 == 1); touch(&mut counter); point }).consume() as Int;
            counter
        }";
    let mut map = SourceMap::default();
    let file = map.add("receiver_cache.lc", source);
    let source = map.get(file);
    let parsed = parse(source);
    assert!(parsed.is_success());
    let (first, stored) = elaborate_with_store(source, &parsed.program, ProofStore::new());
    assert!(first.is_success(), "{:#?}", first.diagnostics);
    assert_eq!(first.holes.len(), 1);
    assert_eq!(stored.stats().searches, 1);
    assert_eq!(stored.len(), 1);
    let serialized = stored.render("receiver_cache.lc");
    let (mut lock, warnings) = locus::store::Lockfile::parse(&serialized).unwrap();
    let loaded = lock.take("receiver_cache.lc");
    assert!(warnings.is_empty());
    let (second, replayed) = elaborate_with_store(source, &parsed.program, loaded.locked(true));
    assert!(second.is_success(), "{:#?}", second.diagnostics);
    assert_eq!(second.holes.len(), 1);
    assert_eq!(replayed.stats().searches, 0);
    assert_eq!(replayed.stats().hits, 1);
    assert_eq!(serialized, replayed.render("receiver_cache.lc"));
    let rust = locus::erased::print_module(first.session.erased());
    assert_eq!(rust, locus::erased::print_module(second.session.erased()));
    let function = first.function("run").unwrap();
    let actual = Interpreter::new(first.session.erased(), 10000)
        .call(function, vec![])
        .unwrap();
    let checked = locus::exec::CheckInterpreter::new(first.session.program(), 10000)
        .with_lending(first.session.lending())
        .call(function, vec![])
        .unwrap();
    assert_eq!(actual, checked);
    assert_eq!(actual.debug(first.session.erased()), "1");
    let directory =
        std::env::temp_dir().join(format!("locus-receiver-probe-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("main.rs"),
        format!("{rust}\nfn main() {{ println!(\"{{}}\", run()); }}"),
    )
    .unwrap();
    let compiled = std::process::Command::new("rustc")
        .args(["--edition=2024", "-Dwarnings"])
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
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "1");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn model_signature_preserves_the_declared_physical_source_shape() {
    for (declared, parameter) in [
        ("[u8; 1]", "[u8; 2]"),
        ("Vec<u8>", "[u8]"),
        ("[u8]", "Vec<u8>"),
    ] {
        let result = heap_check(&format!(
            "impl Model<{declared}> for Int {{ logic fn model(source: &{parameter}) -> Self {{ 0 }} }}"
        ));
        assert!(
            result.diagnostics.iter().any(|d| d.code == "L0282"),
            "{declared}, {parameter}: {:#?}",
            result.diagnostics
        );
    }
}
