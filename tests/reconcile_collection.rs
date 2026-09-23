//! The tier-three design test: explicit evidence through a real mutable collection.
use locus::{
    elab::{Elaborated, Options, elaborate_with_options},
    preview::{Feature, Status},
    source::SourceMap,
};
const COLLECTION: &str = include_str!("fixtures/verified_collection.lc");
fn check(body: &str) -> Elaborated {
    let text = format!(
        "{}\n{}\n{body}",
        include_str!("../library/logical.lc"),
        include_str!("../library/buffer_model.lc")
    );
    let mut sources = SourceMap::default();
    let id = sources.add("verified_collection.lc", text);
    let source = sources.get(id);
    let parsed = locus::parser::parse(source);
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let mut options = Options::default();
    for f in [Feature::LogicalData, Feature::HeapViews] {
        if f.status() == Status::Preview {
            options.previews.enable(f.name()).unwrap();
        }
    }
    elaborate_with_options(source, &parsed.program, &options)
}
fn accepted(text: &str) -> Elaborated {
    let result = check(text);
    assert!(result.is_success(), "{:?}", result.diagnostics);
    locus::erased::check_module(result.session.erased()).unwrap();
    result
}
fn run(text: &str) {
    let result = accepted(text);
    let function = result.function("run").unwrap();
    let checked = locus::exec::CheckInterpreter::new(result.session.program(), 100000)
        .with_lending(result.session.lending())
        .call(function, vec![])
        .unwrap();
    let erased = locus::erased::Interpreter::new(result.session.erased(), 100000)
        .call(function, vec![])
        .unwrap();
    assert_eq!(checked, erased);
    assert_eq!(
        erased,
        locus::erased::Outcome::Value(locus::erased::Value::u8(8))
    );
    let rust = locus::erased::print_module(result.session.erased());
    assert!(
        rust.contains("Box::new("),
        "physical allocation was erased: {rust}"
    );
    assert!(
        rust.contains("tick(&mut counter)"),
        "executable logical-result call was erased: {rust}"
    );
    let dir = std::env::temp_dir().join(format!(
        "locus-verified-collection-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("main.rs");
    std::fs::write(&file, format!("{rust}\nfn main(){{assert_eq!(run(),8);}}")).unwrap();
    let out = std::process::Command::new("rustc")
        .args(["--edition=2024", "-Dwarnings"])
        .arg(file)
        .arg("-o")
        .arg(dir.join("main"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}\n{rust}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        std::process::Command::new(dir.join("main"))
            .status()
            .unwrap()
            .success()
    );
    let _ = std::fs::remove_dir_all(dir);
}
#[test]
fn verified_collection_proofs_snapshots_and_effects_agree_three_ways() {
    run(COLLECTION);
}
#[test]
fn harmless_binding_refactor_preserves_the_interface_and_proof() {
    run(&COLLECTION.replace(
        "    items.push(value);",
        "    let inserted = value;\n    items.push(inserted);",
    ));
}
#[test]
fn wrong_append_evidence_is_refused_at_the_explicit_slot() {
    let result = check(&COLLECTION.replace(
        "    let exactly_one: @(items.len() == 1) = added;",
        "    let exactly_one: @(items.len() == 2) = added;",
    ));
    assert!(!result.is_success());
    assert!(
        result.diagnostics.iter().any(|d| matches!(d.code, "L0230")),
        "{:?}",
        result.diagnostics
    );
}
#[test]
fn existential_witness_may_rebuild_evidence_but_not_escape_as_data() {
    accepted(
        "logic fn transport(predicate: logic Fn(n:Int)->Prop, proof:@Exists::<Int>(predicate))->@Exists::<Int>(predicate) {match proof {Exists::Witness(n) @ found=>Exists::<Int>::Witness(n) @ found}}",
    );
    let result = check(
        "logic fn extract(predicate: logic Fn(n:Int)->Prop, proof:@Exists::<Int>(predicate))->Int {match proof {Exists::Witness(n) @ found=>n}}",
    );
    assert!(!result.is_success());
    assert!(
        result.diagnostics.iter().any(|d| d.code == "L0227"),
        "{:?}",
        result.diagnostics
    );
}
