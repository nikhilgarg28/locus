use locus::{elab, project};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn scratch(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("facades_{name}"));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}
fn generate(path: &Path) -> Result<String, String> {
    project::rust(project::check(path, &elab::Options::default()).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/modules/proof-output")
}
fn compile(dir: &Path, source: &str, mode: &str) -> Output {
    fs::write(dir.join("main.rs"), source).unwrap();
    Command::new("rustc")
        .args([
            "--edition=2024",
            "-Dwarnings",
            "-C",
            &format!("overflow-checks={mode}"),
        ])
        .arg(dir.join("main.rs"))
        .arg("-o")
        .arg(dir.join("run"))
        .output()
        .unwrap()
}
fn run(dir: &Path, rust: &str, driver: &str) {
    fs::write(dir.join("generated.rs"), rust).unwrap();
    for mode in ["yes", "no"] {
        let out = compile(dir, &format!("include!(\"generated.rs\");\n{driver}"), mode);
        assert!(
            out.status.success(),
            "{}\n{rust}",
            String::from_utf8_lossy(&out.stderr)
        );
        let out = Command::new(dir.join("run")).output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
#[doc = "spec: 1.28:18, 1.28:19, 1.28:14"]
fn projected_results_methods_aliases_and_internal_proof_calls_execute() {
    let dir = scratch("shapes");
    let rust = generate(&fixture()).unwrap();
    assert!(rust.contains("__locus_with_proofs_"), "{rust}");
    assert!(rust.contains("-> (u8, Erased)"), "{rust}");
    run(
        &dir,
        &rust,
        r#"
fn main() {
    let _: () = proof_only();
    let _: () = all_proofs();
    let _: () = unit();
    assert_eq!(mixed(), (7u8, true));
    assert_eq!(nested(), (7u8, true));
    assert_eq!(singleton(), (7u8,));
    assert_eq!(physical(), ((7u8,), ()));
    for n in 0..=255u8 {
        assert_eq!(increment(n), n.wrapping_add(1));
        assert_eq!(aliases::also_increment(n), n.wrapping_add(1));
        assert_eq!(reuse(n), n.wrapping_add(1));
    }
    let mut c = Counter::new(3);
    assert_eq!(c.read(), 3);
    assert_eq!(c._read(), 3);
    let _: () = c.advance();
    assert_eq!(c.twice(), 5);
    assert_eq!(c.consume(), 5);
    assert_eq!(loop_value(), 8);
    assert_eq!(chained(9), 10);
    let b = Box::new(5u8);
    let address = (&*b) as *const u8;
    let b = owned(b);
    assert_eq!((&*b) as *const u8, address);
    assert!(std::ptr::eq(borrowed(&b), &*b));
}
"#,
    );
}

#[test]
#[doc = "spec: 1.28:19, 1.18:2"]
fn wrappers_preserve_exactly_once_mutation_and_writes_before_panic() {
    let dir = scratch("effects");
    let rust = generate(&fixture()).unwrap();
    run(
        &dir,
        &rust,
        r#"
fn main() {
    let mut value = 0u8;
    assert_eq!(step(&mut value), 1);
    assert_eq!(value, 1);
    assert_eq!(step(&mut value), 9);
    assert_eq!(value, 2);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fail(&mut value)));
    let error = result.unwrap_err();
    let message = error.downcast_ref::<String>().map(String::as_str)
        .or_else(|| error.downcast_ref::<&str>().copied()).unwrap();
    assert_eq!(message, "facade panic");
    assert_eq!(value, 42);
    let _: () = clear(&mut value);
    assert_eq!(value, 0);
}
"#,
    );
}

#[test]
#[doc = "spec: 1.28:13, 1.17:1"]
fn projection_does_not_open_inputs_fields_containers_callbacks_or_logical_data() {
    let dir = scratch("rejections");
    let entry = dir.join("export.lc");
    for (source, path) in [
        ("pub fn bad(p: @(1 == 1)) -> @(1 == 1) {p}", "parameter `p`"),
        (
            "pub fn bad(p: (u8, @(1 == 1))) -> u8 {p.0}",
            "tuple field 1",
        ),
        (
            "pub fn bad(callback: fn(u8) -> @(1 == 1)) -> u8 {0}",
            "parameter",
        ),
        (
            "pub fn bad() -> (@(1 == 1), u8, Nat) {(_, 1, 1)}",
            "tuple field 2",
        ),
        ("pub fn bad() -> Nat {1}", "result"),
        ("pub fn bad() -> (Nat, @(1 == 1)) {(1, _)}", "result"),
        ("pub fn bad() -> Prop {prop!(true)}", "result"),
        ("pub struct Bad {pub proof: @(1 == 1)}", "field `proof`"),
        ("pub struct Bad {pub n: u8, proof: @(n == 1)}", "field `n`"),
        ("pub enum Bad { Proof(@(1 == 1)) }", "variant `Proof`"),
        (
            "pub fn bad() -> Option<@(1 == 1)> {Some(prove!(1 == 1))}",
            "variant",
        ),
        (
            "pub fn bad() -> Box<@(1 == 1)> {Box::new(prove!(1 == 1))}",
            "element",
        ),
        (
            "pub struct Bad {} impl Bad {pub fn bad(&self, p: @(1 == 1)) -> @(1 == 1) {p}}",
            "parameter `p`",
        ),
    ] {
        fs::write(&entry, source).unwrap();
        let err = generate(&entry).unwrap_err();
        assert!(
            err.contains("L0504") && err.contains(path),
            "{source}\n{err}"
        );
        assert!(err.contains("export.lc"), "{err}");
    }
}

#[test]
#[doc = "spec: 1.28:19, 1.28:14"]
fn raw_proof_implementations_are_private_to_same_crate_and_downstream_rust() {
    let dir = scratch("privacy");
    let rust = generate(&fixture()).unwrap();
    fs::write(dir.join("generated.rs"), &rust).unwrap();
    let raw = rust
        .lines()
        .find(|l| l.contains("fn __locus_with_proofs_") && l.contains("certify("))
        .unwrap()
        .split("fn ")
        .nth(1)
        .unwrap()
        .split('(')
        .next()
        .unwrap();
    for attack in [
        format!("let _=__locus_impl::{raw}(1);"),
        "let _=Counter::__locus_with_proofs_read;".into(),
        "let _=Counter::__locus_with_proofs_new;".into(),
        "let c=Counter::new(1); let _=c.__locus_with_proofs_consume();".into(),
        "let _=__locus_impl::Erased;".into(),
    ] {
        let out = compile(
            &dir,
            &format!("include!(\"generated.rs\");fn main(){{{attack}}}"),
            "yes",
        );
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            !out.status.success() && (err.contains("E0603") || err.contains("E0624")),
            "{attack}\n{err}"
        );
    }
    let out = Command::new("rustc")
        .args([
            "--edition=2024",
            "-Dwarnings",
            "--crate-type=lib",
            "--crate-name=verified",
        ])
        .arg(dir.join("generated.rs"))
        .arg("-o")
        .arg(dir.join("libverified.rlib"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    for (body, succeeds) in [
        ("assert_eq!(verified::increment(1),2);", true),
        ("let _=verified::Counter::__locus_with_proofs_read;", false),
    ] {
        fs::write(dir.join("consumer.rs"), format!("fn main(){{{body}}}")).unwrap();
        let out = Command::new("rustc")
            .args(["--edition=2024", "-Dwarnings", "--extern"])
            .arg(format!(
                "verified={}",
                dir.join("libverified.rlib").display()
            ))
            .arg(dir.join("consumer.rs"))
            .arg("-o")
            .arg(dir.join("consumer"))
            .output()
            .unwrap();
        assert_eq!(
            out.status.success(),
            succeeds,
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
#[doc = "spec: 3.5:4, 1.28:19"]
fn facade_examples_agree_with_both_source_execution_paths() {
    use locus::erased::{Interpreter, Outcome, Overflow, Value};
    use locus::exec::CheckInterpreter;
    let unit = project::check(&fixture(), &Default::default()).unwrap();
    let session = &unit.checked.session;
    let functions = session.erased().fns.as_slice();
    let find = |name: &str| {
        functions
            .iter()
            .find(|f| f.name.ends_with(&format!("_{name}")))
            .unwrap()
            .reference
    };
    for mode in Overflow::ALL {
        for byte in 0..=255u8 {
            let expected = Outcome::Value(Value::Tuple(vec![
                Value::u8(byte.wrapping_add(1)),
                Value::Proved,
            ]));
            assert_eq!(
                CheckInterpreter::new(session.program(), 10000)
                    .with_overflow(mode)
                    .call(find("certify"), vec![Value::u8(byte)])
                    .unwrap(),
                expected
            );
            assert_eq!(
                Interpreter::new(session.erased(), 10000)
                    .with_overflow(mode)
                    .call(find("certify"), vec![Value::u8(byte)])
                    .unwrap(),
                expected
            );
        }
        for (name, input, outcome, after) in [
            ("clear", 99, Outcome::Value(Value::Proved), 0),
            (
                "step",
                0,
                Outcome::Value(Value::Tuple(vec![Value::u8(1), Value::Proved])),
                1,
            ),
            (
                "step",
                1,
                Outcome::Value(Value::Tuple(vec![Value::u8(9), Value::Proved])),
                2,
            ),
            ("fail", 0, Outcome::Panic("facade panic".into()), 42),
        ] {
            let expected = (outcome, vec![Value::u8(after)]);
            assert_eq!(
                CheckInterpreter::new(session.program(), 10000)
                    .with_overflow(mode)
                    .with_lending(session.lending())
                    .call_lending(find(name), vec![Value::u8(input)])
                    .unwrap(),
                expected
            );
            assert_eq!(
                Interpreter::new(session.erased(), 10000)
                    .with_overflow(mode)
                    .call_lending(find(name), vec![Value::u8(input)])
                    .unwrap(),
                expected
            );
        }
    }
}

#[test]
#[doc = "spec: 1.28:16, 1.28:18"]
fn projected_builds_are_deterministic_and_receipts_track_erased_claim_changes() {
    let dir = scratch("receipts");
    let entry = dir.join("export.lc");
    fs::write(&entry, "pub fn answer() -> (u8, @(1 == 1)) {(7, _)}").unwrap();
    let build = project::Build::new(&entry)
        .offline(true)
        .out_dir(dir.join("out"));
    let built = build.generate().unwrap();
    let rust = fs::read(&built.rust).unwrap();
    let receipt = fs::read(&built.receipt).unwrap();
    build.generate().unwrap();
    assert_eq!(fs::read(&built.rust).unwrap(), rust);
    assert_eq!(fs::read(&built.receipt).unwrap(), receipt);
    assert!(build.is_current().unwrap());
    fs::write(&entry, "pub fn answer() -> (u8, @(2 == 2)) {(7, _)}").unwrap();
    assert!(!build.is_current().unwrap());
    build.generate().unwrap();
    assert_eq!(fs::read(&built.rust).unwrap(), rust);
    assert_ne!(fs::read(&built.receipt).unwrap(), receipt);
    assert!(build.is_current().unwrap());
    fs::write(&entry, "pub fn answer() -> (u8, @(1 == 2)) {(7, _)}").unwrap();
    assert!(build.generate().is_err());
    assert!(!build.is_current().unwrap());
    assert_eq!(fs::read(&built.rust).unwrap(), rust);
}
