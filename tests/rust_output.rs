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

    let module = session.erased();
    assert_eq!(check_module(module), Ok(()));
    let mut source = print_module(module);

    // The output reads like the source.
    for expected in [
        "pub fn increment(n: u8) -> (u8, Proved) {",
        "let out = n.wrapping_add(1_u8);",
        "(out, Proved)",
        "pub enum Classified {",
        "Zero(u8, Proved),",
        "if n != 0_u8 {",
        "Classified::NonZero(n, Proved)",
        "match classify(m) {",
        "Classified::Zero(v, h) => {",
        "pub fn bounded_walk(limit: u8) -> (u8, Proved) {",
        "let mut i = 0_u8;",
        "loop {",
        "break (i, Proved)",
        "i = i.wrapping_add(1_u8);",
        "for i in 0_u8..n {",
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
