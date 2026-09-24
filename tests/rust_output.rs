//! Compiled versus interpreted (README milestone 1): the Rust printed from
//! the erased tree is compiled with rustc, run, and compared with the
//! reference interpreter, line for line.

mod common;

use std::path::PathBuf;
use std::process::Command;

use common::*;
use locus::erased::{Interpreter, Value, check_module, print_module};
use locus::typed::FnRef;

const FUEL: u64 = 200_000;
const INPUTS: [u8; 6] = [0, 1, 7, 100, 200, 255];

fn renamed(mut item: locus::typed::FnItem, name: &str) -> locus::typed::FnItem {
    item.name = name.into();
    item
}

#[test]
fn generated_rust_compiles_and_agrees_with_the_interpreter() {
    let (mut session, prelude, theory) = setup();
    let classified = session.declare_enum(&classified_enum(prelude)).unwrap();
    let increment_ref = session.declare_fn(&increment(false)).unwrap();
    let increment_id = exec_id(increment_ref);
    let classify_ref = session.declare_fn(&classify(classified)).unwrap();
    let mut programs: Vec<(&str, FnRef)> =
        vec![("increment", increment_ref), ("classify", classify_ref)];
    let mut declare = |name: &'static str, item: locus::typed::FnItem| {
        let reference = session.declare_fn(&renamed(item, name)).unwrap();
        programs.push((name, reference));
    };
    declare("increment_math", increment(true));
    declare("preserve", preserve(theory, false, true));
    declare("preserve_math", preserve(theory, true, true));
    declare("bounded_walk", bounded_walk(theory, true));
    declare("count", counting_loop(false, None));
    declare("count_by_calls", counting_loop(false, Some(increment_id)));
    declare(
        "zero_or_self",
        zero_or_self(prelude, classified, exec_id(classify_ref)),
    );
    declare("note", snapshot_note());

    let module = session.erased();
    assert_eq!(check_module(module), Ok(()));
    let mut source = print_module(module);

    // A `Ghost<T>` binding leaves nothing behind.
    assert!(!source.contains("let g"), "{source}");

    // The output reads like the source.
    for expected in [
        "pub fn increment(n: u8) -> (u8, Erased) {",
        "pub fn note(n: u8) -> u8 {",
        "let out = n.wrapping_add(1_u8);",
        "(out, Erased)",
        "pub enum Classified {",
        "Zero(u8, Erased),",
        "if n != 0_u8 {",
        "Classified::NonZero(n, Erased)",
        "match classify(m) {",
        "Classified::Zero(v, _) => {",
        "pub fn bounded_walk(limit: u8) -> (u8, Erased) {",
        "let mut i = 0_u8;",
        "loop {",
        "break (i, Erased)",
        "i = i.wrapping_add(1_u8);",
        "for _i in 0_u8..n {",
        "acc = increment(acc).0;",
    ] {
        assert!(source.contains(expected), "missing: {expected}\n{source}");
    }

    // A main that prints every function at every input.
    let mut expected_output = String::new();
    source.push_str("\nfn main() {\n");
    for (name, reference) in &programs {
        for input in INPUTS {
            source.push_str(&format!("    println!(\"{{:?}}\", {name}({input}));\n"));
            let value = Interpreter::new(module, FUEL)
                .call(*reference, vec![Value::u8(input)])
                .unwrap();
            expected_output.push_str(&value.debug(module));
            expected_output.push('\n');
        }
    }
    source.push_str("}\n");

    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let source_path = directory.join("locus_generated.rs");
    let binary_path = directory.join("locus_generated");
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

fn printed_source(text: &str) -> (locus::elab::Elaborated, String) {
    let mut sources = locus::source::SourceMap::default();
    let file = sources.add("cleanup.lc", text);
    let source = sources.get(file);
    let parsed = locus::parser::parse(source);
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let checked = locus::elab::elaborate(source, &parsed.program);
    assert!(checked.is_success(), "{:?}", checked.diagnostics);
    let printed = print_module(checked.session.erased());
    (checked, printed)
}

fn compile_and_run_cleanup(name: &str, source: &str, main: &str) -> String {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let source_path = directory.join(format!("locus_cleanup_{name}.rs"));
    let binary_path = directory.join(format!("locus_cleanup_{name}"));
    std::fs::write(&source_path, format!("{source}\n{main}\n")).unwrap();
    let output = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
        .args(["--edition", "2021", "-D", "warnings", "-o"])
        .arg(&binary_path)
        .arg(&source_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{source}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(binary_path).output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
#[doc = "spec: 1.18:3"]
#[doc = "spec: 3.5:3"]
fn cascading_dead_markers_disappear_but_the_mutating_initializer_stays() {
    let (checked, source) = printed_source(
        r#"
fn mutate(counter: &mut u8) -> @(0u8 == 0u8) {
    counter = counter.wrapping_add(1);
    _
}
fn cleaned() -> u8 {
    let a: @(0u8 == 0u8) = _;
    let b = a;
    let c = b;
    let mut counter = 0u8;
    let evidence = mutate(&mut counter);
    counter
}
"#,
    );
    assert!(!source.contains("let a ="), "{source}");
    assert!(!source.contains("let b ="), "{source}");
    assert!(!source.contains("let c ="), "{source}");
    assert!(!source.contains("let evidence"), "{source}");
    assert!(source.contains("let _ = mutate(&mut counter);"), "{source}");
    assert!(!source.contains("#![allow("), "{source}");
    let reference = checked.function("cleaned").unwrap();
    let result = Interpreter::new(checked.session.erased(), FUEL)
        .call(reference, vec![])
        .unwrap();
    assert_eq!(result, locus::erased::Outcome::Value(Value::u8(1)));
    assert_eq!(
        compile_and_run_cleanup(
            "effect",
            &source,
            "fn main() { println!(\"{}\", cleaned()); }"
        ),
        "1\n"
    );
}

#[test]
fn unused_parameters_and_patterns_keep_their_signatures_and_runtime_values() {
    let (_, source) = printed_source(
        r#"
struct Resource { value: u8 }
fn identity(value: u8, evidence: @(0u8 == 0u8)) -> u8 { value }
fn hold(resource: Resource) -> u8 { let owned = resource; 7 }
fn discard() -> u8 {
    let (value, evidence) = (3u8, prove!(0u8 == 0u8));
    value
}
"#,
    );
    assert!(
        source.contains("fn identity(value: u8, _evidence: Erased) -> u8"),
        "{source}"
    );
    assert!(source.contains("let _owned = resource;"), "{source}");
    assert!(
        source.contains("let (value, _) = (3_u8, Erased);"),
        "{source}"
    );
    let main = r#"
impl Drop for Resource {
    fn drop(&mut self) { println!("drop {}", self.value); }
}
fn main() { println!("{}", hold(Resource { value: 9 })); println!("{}", discard()); }
"#;
    assert_eq!(
        compile_and_run_cleanup("bindings", &source, main),
        "drop 9\n7\n3\n"
    );
}

#[test]
#[doc = "spec: 1.18:3"]
fn runtime_only_output_has_no_marker_support_or_blanket_lint_allow() {
    let (_, source) = printed_source("fn unchanged(n: u8) -> u8 { n }");
    assert!(!source.contains("struct Erased"), "{source}");
    assert!(!source.contains("struct Ghost"), "{source}");
    assert!(!source.contains("#[allow("), "{source}");
    assert_eq!(
        compile_and_run_cleanup(
            "plain",
            &source,
            "fn main() { println!(\"{}\", unchanged(3)); }"
        ),
        "3\n"
    );
}

#[test]
fn a_dead_marker_initializer_still_panics_before_later_work() {
    let (_, source) = printed_source(
        r#"
fn fail() -> @(0u8 == 0u8) { panic!("evidence failed") }
fn run() -> u8 { let discarded = fail(); 7 }
"#,
    );
    assert!(source.contains("let _ = fail();"), "{source}");
    let main = r#"fn main() { std::panic::set_hook(Box::new(|_| {})); println!("{}", std::panic::catch_unwind(run).is_err()); }"#;
    assert_eq!(compile_and_run_cleanup("panic", &source, main), "true\n");
}

#[test]
fn unused_binding_names_do_not_shadow_existing_underscore_names() {
    let (_, source) = printed_source(
        "fn keep(_value: u8, value: u8) -> u8 { let _local = _value; let local = value; _local }",
    );
    assert_eq!(
        compile_and_run_cleanup(
            "names",
            &source,
            "fn main() { println!(\"{}\", keep(3, 9)); }"
        ),
        "3\n"
    );
}

#[test]
fn a_dead_marker_store_keeps_the_effectful_right_hand_side() {
    let (_, source) = printed_source(
        r#"
fn bump(value: &mut u8) -> @(0u8 == 0u8) { value = value.wrapping_add(1); _ }
fn run() -> u8 {
    let mut count = 0u8;
    let mut evidence = prove!(0u8 == 0u8);
    evidence = bump(&mut count);
    count
}
"#,
    );
    assert!(!source.contains("let mut evidence"), "{source}");
    assert!(!source.contains("evidence ="), "{source}");
    assert!(source.contains("let _ = bump(&mut count);"), "{source}");
    assert_eq!(
        compile_and_run_cleanup("store", &source, "fn main() { println!(\"{}\", run()); }"),
        "1\n"
    );
}

#[test]
fn source_names_cannot_collide_with_the_generated_marker() {
    for text in [
        "struct Erased { value: u8 }",
        "fn Erased() -> u8 { 0 }",
        "fn run(Erased: u8) -> u8 { 0 }",
        "fn run() -> u8 { let Erased = 0u8; 0 }",
    ] {
        let mut sources = locus::source::SourceMap::default();
        let file = sources.add("collision.lc", text);
        let parsed = locus::parser::parse(sources.get(file));
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "L0008"),
            "{text}\n{:?}",
            parsed.diagnostics
        );
    }
}

#[test]
fn divergence_is_kept_without_an_unreachable_erased_tail() {
    let (_, source) = printed_source("fn spin_evidence(x: u8) -> @(x == x) { loop {} }");
    assert!(!source.contains("let _ = loop"), "{source}");
    assert_eq!(
        compile_and_run_cleanup("divergence", &source, "fn main() {}"),
        ""
    );
}

#[test]
fn physical_allocation_feeding_only_a_model_observation_is_retained() {
    let (_, source) = printed_source(
        "fn run()->u8{let boxed=Box::new(7u8);let observed=model!(*boxed) as Int;8}",
    );
    assert!(
        source.contains("Box::new(7_u8)"),
        "physical allocation disappeared: {source}"
    );
    assert_eq!(
        compile_and_run_cleanup(
            "model_allocation",
            &source,
            "fn main(){println!(\"{}\",run());}"
        ),
        "8\n"
    );
}
